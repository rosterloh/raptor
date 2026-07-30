use crate::components::ui::{Button, ButtonVariant, Card, Dialog};
use crate::components::*;
use crate::pages::{EntityTags, TagKind};
use crate::{api, logic, Route};
use dioxus::prelude::*;

#[component]
pub fn TargetDetail(cid: String) -> Element {
    let cid_s = use_signal(|| cid.clone());
    let mut target = use_resource(move || async move { api::get_target(&cid_s()).await });
    let attributes = use_resource(move || async move { api::target_attributes(&cid_s()).await });
    let mut assigned = use_resource(move || async move { api::assigned_ds(&cid_s()).await });
    let installed = use_resource(move || async move { api::installed_ds(&cid_s()).await });
    let mut actions =
        use_resource(move || async move { api::target_actions(&cid_s(), 0, 10).await });
    let tags = use_resource(move || async move { api::target_tags(&cid_s()).await });
    use_polling(actions);

    let mut show_assign = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();

    let mut refresh = move || {
        target.restart();
        assigned.restart();
        actions.restart();
    };

    rsx! {
        h1 { class: HEADING, "Target {cid_s()}" }
        div { class: "mb-4 flex gap-2",
            Button { onclick: move |_| show_assign.set(true), "Assign distribution set" }
            Button {
                variant: ButtonVariant::Destructive,
                onclick: move |_| confirm_delete.set(true),
                "Delete target"
            }
        }
        div { class: "grid grid-cols-2 gap-4",
            Card {
                h2 { class: "mb-2 font-semibold text-foreground", "Status" }
                match &*target.read_unchecked() {
                    Some(Ok(t)) => rsx! {
                        dl { class: "space-y-1 text-sm",
                            Row { k: "Name", v: t.name.clone() }
                            div { class: "flex gap-2",
                                dt { class: "w-32 text-muted-foreground", "Update status" }
                                dd { StatusBadge { status: t.update_status.clone() } }
                            }
                            Row { k: "Last poll", v: t.last_controller_request_at.map(logic::format_ts).unwrap_or_else(|| "never".into()) }
                            Row { k: "Address", v: t.address.clone().unwrap_or_else(|| "-".into()) }
                            Row { k: "Security token", v: t.security_token.clone() }
                        }
                    },
                    Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| target.restart() } },
                    None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
                }
            }
            Card {
                h2 { class: "mb-2 font-semibold text-foreground", "Distribution sets" }
                DsSummary { label: "Assigned", res: assigned }
                DsSummary { label: "Installed", res: installed }
            }
            Card {
                EntityTags { kind: TagKind::Target, entity_key: cid_s(), tags }
            }
            Card {
                h2 { class: "mb-2 font-semibold text-foreground", "Attributes" }
                match &*attributes.read_unchecked() {
                    Some(Ok(attrs)) if attrs.is_empty() => rsx! { p { class: "text-sm text-muted-foreground", "none reported" } },
                    Some(Ok(attrs)) => rsx! {
                        dl { class: "space-y-1 text-sm",
                            for (k , v) in attrs.clone() {
                                Row { k: "{k}", v: "{v}" }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                    None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
                }
            }
            Card {
                h2 { class: "mb-2 font-semibold text-foreground", "Recent actions" }
                match &*actions.read_unchecked() {
                    Some(Ok(page)) => rsx! {
                        ul { class: "space-y-2 text-sm",
                            for a in page.content.clone() {
                                li { key: "{a.id}", class: "flex items-center justify-between",
                                    span { "#{a.id} {a.action_type} — {a.detail_status} ({logic::format_ts(a.last_modified_at)})" }
                                    if a.status == "pending" {
                                        button {
                                            class: "text-xs text-err hover:underline",
                                            onclick: move |_| {
                                                let cid = cid_s();
                                                let aid = a.id;
                                                spawn(async move {
                                                    match api::cancel_action(&cid, aid, false).await {
                                                        Ok(()) => toast_ok(format!("cancel requested for action #{aid}")),
                                                        Err(e) => toast_error(e.to_string()),
                                                    }
                                                    actions.restart();
                                                });
                                            },
                                            "Cancel"
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                    None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
                }
            }
        }
        AssignDsDialog { open: show_assign, cid: cid_s, on_done: move |_| refresh() }
        ConfirmDialog {
            title: "Delete target".to_string(),
            message: format!("Delete target {} and its attributes? This cannot be undone.", cid_s()),
            open: confirm_delete,
            on_confirm: move |_| {
                let cid = cid_s();
                spawn(async move {
                    match api::delete_target(&cid).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {cid}"));
                            nav.push(Route::Targets {});
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

#[component]
fn Row(k: String, v: String) -> Element {
    rsx! {
        div { class: "flex gap-2",
            dt { class: "w-32 shrink-0 text-muted-foreground", "{k}" }
            dd { class: "break-all", "{v}" }
        }
    }
}

#[component]
fn DsSummary(
    label: String,
    res: Resource<crate::api::ApiResult<Option<raptor_api_types::DsRest>>>,
) -> Element {
    rsx! {
        div { class: "mb-2 text-sm",
            span { class: "text-muted-foreground", "{label}: " }
            match &*res.read_unchecked() {
                Some(Ok(Some(ds))) => rsx! {
                    Link { to: crate::Route::DsDetail { id: ds.id }, class: "text-primary hover:underline",
                        "{ds.name} {ds.version}"
                    }
                },
                Some(Ok(None)) => rsx! { span { class: "text-muted-foreground", "none" } },
                Some(Err(e)) => rsx! { span { class: "text-err", "{e}" } },
                None => rsx! { span { class: "text-muted-foreground", "…" } },
            }
        }
    }
}

/// Distribution-set picker with forced/soft choice.
#[component]
pub fn AssignDsDialog(
    open: Signal<bool>,
    cid: Signal<String>,
    on_done: EventHandler<()>,
) -> Element {
    let sets = use_resource(move || async move {
        if open() {
            api::list_ds(0, 100, None).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(|| None::<i64>);
    let mut forced = use_signal(|| true);
    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Assign distribution set" }
            match &*sets.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "text-sm text-muted-foreground", "No distribution sets yet." }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for ds in page.content.clone() {
                            li { key: "{ds.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "radio",
                                        name: "ds",
                                        checked: selected() == Some(ds.id),
                                        onchange: move |_| selected.set(Some(ds.id)),
                                    }
                                    span { "{ds.name} {ds.version}" }
                                    if !ds.complete {
                                        span { class: "text-xs text-pend", "(incomplete)" }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
            }
            label { class: "mb-4 flex items-center gap-2 text-sm",
                input {
                    r#type: "checkbox",
                    checked: forced(),
                    onchange: move |e| forced.set(e.checked()),
                }
                "Forced (device installs immediately)"
            }
            div { class: "flex justify-end gap-2",
                button {
                    class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
                Button {
                    disabled: selected().is_none(),
                    onclick: move |_| {
                        let (cid, ds_id, is_forced) = (cid(), selected().unwrap(), forced());
                        spawn(async move {
                            match api::assign_ds(&cid, ds_id, is_forced).await {
                                Ok(r) if r.assigned > 0 => toast_ok("assignment created"),
                                Ok(_) => toast_ok("already assigned"),
                                Err(e) => {
                                    toast_error(e.to_string());
                                    return;
                                }
                            }
                            open.set(false);
                            on_done.call(());
                        });
                    },
                    "Assign"
                }
            }
        }
    }
}
