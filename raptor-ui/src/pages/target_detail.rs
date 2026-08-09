use crate::components::ui::{Button, ButtonVariant, Dialog};
use crate::components::*;
use crate::pages::{EntityTags, TagKind};
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::{ActionRest, DsRest};

const ACTION_ROWS: u64 = 20;
/// A single action's history is short — assignment through to finished is a
/// handful of entries even on a device that retried.
const STATUS_ROWS: u64 = 50;

#[component]
pub fn TargetDetail(cid: String) -> Element {
    let cid_s = use_signal(|| cid.clone());
    let mut target = use_resource(move || async move { api::get_target(&cid_s()).await });
    let attributes = use_resource(move || async move { api::target_attributes(&cid_s()).await });
    let mut assigned = use_resource(move || async move { api::assigned_ds(&cid_s()).await });
    let installed = use_resource(move || async move { api::installed_ds(&cid_s()).await });
    let mut actions =
        use_resource(move || async move { api::target_actions(&cid_s(), 0, ACTION_ROWS).await });
    let tags = use_resource(move || async move { api::target_tags(&cid_s()).await });
    use_polling(actions);

    let mut show_assign = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let tab = use_signal(|| 0usize);
    let nav = use_navigator();
    let now = now_ms();

    let mut refresh = move || {
        target.restart();
        assigned.restart();
        actions.restart();
    };

    rsx! {
        div { class: "mb-5 flex flex-wrap items-end justify-between gap-4",
            div {
                match &*target.read_unchecked() {
                    Some(Ok(t)) => rsx! {
                        h1 { class: "font-display text-3xl font-bold tracking-wider uppercase text-foreground",
                            "{t.name}"
                        }
                        p { class: "mt-0.5 flex flex-wrap items-center gap-2 font-mono text-xs text-muted-foreground",
                            span { class: "text-foreground", "{t.controller_id}" }
                            "·"
                            StatusBadge { status: t.update_status.clone() }
                        }
                    },
                    _ => rsx! {
                        h1 { class: "font-display text-3xl font-bold tracking-wider uppercase text-foreground",
                            "{cid_s()}"
                        }
                    },
                }
            }
            div { class: "flex flex-wrap gap-2",
                Button {
                    variant: ButtonVariant::Outline,
                    onclick: move |_| confirm_delete.set(true),
                    "Delete target"
                }
                Button { onclick: move |_| show_assign.set(true), "Assign update" }
            }
        }

        SectionRule { label: "Delivery state" }
        div { class: "grid grid-cols-1 gap-px border border-border-soft bg-border-soft sm:grid-cols-3",
            // Version alone means nothing across device classes, so each tile
            // names the set the number belongs to.
            DsTile { label: "Installed", res: installed }
            DsTile { label: "Assigned", res: assigned }
            div { class: "tick-scale relative bg-card p-4",
                p { class: "font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                    "Last poll"
                }
                match &*target.read_unchecked() {
                    Some(Ok(t)) => rsx! {
                        p { class: "font-display mt-1.5 text-[28px] leading-none font-bold text-foreground",
                            {logic::relative_age(now, t.last_controller_request_at)}
                        }
                        p { class: "mt-1.5 font-mono text-[11px] text-muted-foreground",
                            {t.last_controller_request_at.map(logic::format_ts).unwrap_or_else(|| "never polled".into())}
                        }
                    },
                    _ => rsx! {
                        p { class: "font-display mt-1.5 text-[28px] leading-none font-bold text-muted-foreground",
                            "…"
                        }
                    },
                }
            }
        }

        SectionRule { label: "Detail" }
        div { class: "border border-border-soft bg-card px-4 pb-4",
            Tabs {
                labels: vec![
                    "Overview".to_string(),
                    "Attributes".to_string(),
                    "Action history".to_string(),
                    "Tags".to_string(),
                ],
                selected: tab,

                TabPanel { index: 0, selected: tab,
                    div { class: "grid grid-cols-1 gap-6 lg:grid-cols-2",
                        div {
                            match &*target.read_unchecked() {
                                Some(Ok(t)) => rsx! {
                                    dl { class: "grid grid-cols-[9rem_1fr] gap-x-4 gap-y-2 text-sm",
                                        Row { k: "Controller ID", v: t.controller_id.clone(), mono: true }
                                        Row { k: "Name", v: t.name.clone() }
                                        Row {
                                            k: "Description",
                                            v: t.description.clone().unwrap_or_else(|| "—".into()),
                                        }
                                        Row {
                                            k: "Address",
                                            v: t.ip_address.clone().or(t.address.clone()).unwrap_or_else(|| "—".into()),
                                            mono: true,
                                        }
                                        Row { k: "Security token", v: t.security_token.clone(), mono: true }
                                        Row { k: "Registered", v: logic::format_ts(t.created_at) }
                                        // ---- target type (issue #34) ----
                                        dt { class: "text-muted-foreground", "Target type" }
                                        dd { class: "break-all text-fg-dim",
                                            TargetTypeField {
                                                cid: cid_s,
                                                target_type_id: t.target_type,
                                                on_changed: move |_| target.restart(),
                                            }
                                        }
                                    }
                                },
                                Some(Err(e)) => rsx! {
                                    ErrorPane { message: e.to_string(), on_retry: move |_| target.restart() }
                                },
                                None => rsx! { p { class: "text-sm text-muted-foreground", "Loading…" } },
                            }
                        }
                        // What is actually on the device. The shape of this list
                        // differs per class — a gateway carries an os module and
                        // an application, a sensor carries one image — which is
                        // why the set's version alone is not the answer.
                        div {
                            p { class: "mb-2 font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                                "Installed modules"
                            }
                            match &*installed.read_unchecked() {
                                Some(Ok(Some(ds))) if !ds.modules.is_empty() => rsx! {
                                    table { class: TABLE,
                                        thead {
                                            tr {
                                                th { class: TH, "Type" }
                                                th { class: TH, "Module" }
                                                th { class: "{TH} text-right", "Version" }
                                            }
                                        }
                                        tbody {
                                            for m in ds.modules.clone() {
                                                tr { key: "{m.id}",
                                                    td { class: "{TD} font-mono text-xs text-muted-foreground",
                                                        "{m.module_type}"
                                                    }
                                                    td { class: TD,
                                                        Link {
                                                            to: Route::ModuleDetail { id: m.id },
                                                            class: LINK_CELL,
                                                            "{m.name}"
                                                        }
                                                    }
                                                    td { class: "{TD} text-right font-mono text-xs", "{m.version}" }
                                                }
                                            }
                                        }
                                    }
                                },
                                Some(Ok(Some(_))) => rsx! {
                                    p { class: "text-sm text-muted-foreground",
                                        "The installed set has no modules — it is incomplete."
                                    }
                                },
                                Some(Ok(None)) => rsx! {
                                    p { class: "text-sm text-muted-foreground",
                                        "Nothing installed yet. The device reports this once it has finished an update."
                                    }
                                },
                                Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                                None => rsx! { p { class: "text-sm text-muted-foreground", "Loading…" } },
                            }
                        }
                    }
                }

                TabPanel { index: 1, selected: tab,
                    match &*attributes.read_unchecked() {
                        Some(Ok(attrs)) if attrs.is_empty() => rsx! {
                            p { class: "text-sm text-muted-foreground",
                                "None reported. A device sends these on its configData request."
                            }
                        },
                        Some(Ok(attrs)) => rsx! {
                            dl { class: "grid grid-cols-[12rem_1fr] gap-x-4 gap-y-2 text-sm",
                                for (k , v) in attrs.clone() {
                                    Row { key: "{k}", k: "{k}", v: "{v}", mono: true }
                                }
                            }
                            p { class: "mt-4 text-xs text-muted-foreground",
                                "Free-form and device-defined, so the keys differ per device class. They are not
                                 queryable: raptor's FIQL covers controllerId, name, description, updateStatus,
                                 lastControllerRequestAt, address and tags, so segmenting by hardware goes through a tag."
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                        None => rsx! { p { class: "text-sm text-muted-foreground", "Loading…" } },
                    }
                }

                TabPanel { index: 2, selected: tab,
                    match &*actions.read_unchecked() {
                        Some(Ok(page)) if page.content.is_empty() => rsx! {
                            p { class: "text-sm text-muted-foreground",
                                "No actions yet. One appears here as soon as a distribution set is assigned."
                            }
                        },
                        Some(Ok(page)) => rsx! {
                            div { class: "divide-y divide-border-subtle",
                                for a in page.content.clone() {
                                    ActionRow {
                                        key: "{a.id}",
                                        cid: cid_s(),
                                        action: a,
                                        on_cancelled: move |_| actions.restart(),
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                        None => rsx! { p { class: "text-sm text-muted-foreground", "Loading…" } },
                    }
                }

                TabPanel { index: 3, selected: tab,
                    EntityTags { kind: TagKind::Target, entity_key: cid_s(), tags }
                }
            }
        }

        AssignDsDialog { open: show_assign, cid: cid_s, on_done: move |_| refresh() }
        ConfirmDialog {
            title: "Delete target".to_string(),
            message: format!(
                "Delete {} and its action history? The device re-registers on its next poll, without its tags or assignment.",
                cid_s(),
            ),
            open: confirm_delete,
            on_confirm: move |_| {
                let cid = cid_s();
                spawn(async move {
                    match api::delete_target(&cid).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {cid}"));
                            nav.push(Route::targets());
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

/// One action, expandable to its status history.
///
/// The history is the answer to "why is this device stuck": the latest
/// `detailStatus` says *what* state it is in, the timeline says how it got there
/// and what the device said on the way. Fetched only when expanded, so a target
/// with twenty actions costs one request until you ask about a specific one.
#[component]
fn ActionRow(cid: String, action: ActionRest, on_cancelled: EventHandler<()>) -> Element {
    let mut open = use_signal(|| false);
    let aid = action.id;
    let cid_for_history = cid.clone();
    let history = use_resource(move || {
        let cid = cid_for_history.clone();
        async move {
            if open() {
                Some(api::action_status_history(&cid, aid, 0, STATUS_ROWS).await)
            } else {
                None
            }
        }
    });
    let cancellable = action.status == "pending";
    rsx! {
        div { class: "py-3",
            div { class: "flex flex-wrap items-center gap-3",
                button {
                    r#type: "button",
                    class: "flex items-center gap-2 font-mono text-xs text-fg-dim hover:text-foreground",
                    aria_expanded: open(),
                    onclick: move |_| open.toggle(),
                    span { class: "inline-block w-3 text-muted-foreground",
                        if open() {
                            "−"
                        } else {
                            "+"
                        }
                    }
                    span { class: "text-foreground", "#{action.id}" }
                }
                span { class: "font-mono text-xs text-muted-foreground", "{action.action_type}" }
                StatusBadge { status: action.detail_status.clone() }
                span { class: "ml-auto font-mono text-[11px] text-muted-foreground",
                    {logic::format_ts(action.last_modified_at)}
                }
                if cancellable {
                    button {
                        r#type: "button",
                        class: "font-mono text-[11px] text-err hover:underline",
                        onclick: move |_| {
                            let cid = cid.clone();
                            spawn(async move {
                                match api::cancel_action(&cid, aid, false).await {
                                    Ok(()) => toast_ok(format!("cancel requested for action #{aid}")),
                                    Err(e) => toast_error(e.to_string()),
                                }
                                on_cancelled.call(());
                            });
                        },
                        "Cancel"
                    }
                }
            }
            if open() {
                div { class: "mt-3 ml-3 border-l border-border-soft pl-4",
                    match &*history.read_unchecked() {
                        Some(Some(Ok(page))) if page.content.is_empty() => rsx! {
                            p { class: "text-xs text-muted-foreground", "No status entries recorded." }
                        },
                        Some(Some(Ok(page))) => rsx! {
                            ol { class: "space-y-3",
                                for s in page.content.clone() {
                                    li { key: "{s.id}", class: "relative",
                                        div { class: "flex flex-wrap items-baseline gap-2",
                                            span { class: "-ml-[21px] size-[7px] shrink-0 rounded-full {tone_fill(logic::status_style(&s.status_type).1)}" }
                                            span { class: "font-mono text-xs text-foreground",
                                                {logic::status_style(&s.status_type).0}
                                            }
                                            span { class: "font-mono text-[11px] text-muted-foreground",
                                                {logic::format_ts(s.reported_at)}
                                            }
                                        }
                                        // Whatever the device attached. Usually the
                                        // only place a failure says why it failed.
                                        if !s.messages.is_empty() {
                                            ul { class: "mt-1 space-y-0.5",
                                                for (i , m) in s.messages.iter().enumerate() {
                                                    li {
                                                        key: "{i}",
                                                        class: "font-mono text-[11px] break-all text-fg-dim",
                                                        "{m}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Some(Err(e))) => rsx! { p { class: "text-xs text-err", "{e}" } },
                        _ => rsx! {
                            p { class: "text-xs text-muted-foreground", "Loading history…" }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn Row(k: String, v: String, #[props(default = false)] mono: bool) -> Element {
    rsx! {
        dt { class: "text-muted-foreground", "{k}" }
        dd {
            class: if mono { "font-mono text-xs break-all text-foreground" } else { "break-all text-fg-dim" },
            "{v}"
        }
    }
}

/// Installed or assigned set: the version big, the set it belongs to underneath.
#[component]
fn DsTile(label: String, res: Resource<api::ApiResult<Option<DsRest>>>) -> Element {
    rsx! {
        div { class: "tick-scale relative bg-card p-4",
            p { class: "font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                "{label}"
            }
            match &*res.read_unchecked() {
                Some(Ok(Some(ds))) => rsx! {
                    p { class: "font-display mt-1.5 text-[28px] leading-none font-bold text-foreground",
                        "{ds.version}"
                    }
                    p { class: "mt-1.5 font-mono text-[11px] text-muted-foreground",
                        Link {
                            to: Route::DsDetail { id: ds.id },
                            class: "hover:text-fg-dim hover:underline",
                            "{ds.name}"
                        }
                        " · {ds.ds_type}"
                    }
                },
                Some(Ok(None)) => rsx! {
                    p { class: "font-display mt-1.5 text-[28px] leading-none font-bold text-muted-foreground",
                        "—"
                    }
                    p { class: "mt-1.5 font-mono text-[11px] text-muted-foreground", "none" }
                },
                Some(Err(e)) => rsx! {
                    p { class: "mt-1.5 text-sm text-err", "{e}" }
                },
                None => rsx! {
                    p { class: "font-display mt-1.5 text-[28px] leading-none font-bold text-muted-foreground",
                        "…"
                    }
                },
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
    // A 400 here means the set's type is incompatible with the target's type
    // (issue #34) — shown inline against the picker rather than as a toast,
    // since it's a validation error the operator can act on right there.
    let mut assign_error = use_signal(|| None::<String>);
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
                                    span { class: "font-mono text-[11px] text-muted-foreground", "{ds.ds_type}" }
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
            if let Some(msg) = assign_error() {
                p { class: "mb-3 text-sm text-err", "{msg}" }
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
                        assign_error.set(None);
                        spawn(async move {
                            match api::assign_ds(&cid, ds_id, is_forced).await {
                                Ok(r) if r.assigned > 0 => toast_ok("assignment created"),
                                Ok(_) => toast_ok("already assigned"),
                                Err(api::ApiError::Server { status: 400, message }) => {
                                    assign_error.set(Some(message));
                                    return;
                                }
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

// ---- target type (issue #34) ----

/// The target's type, if any, with a change/unassign control — mounted
/// alongside the other detail rows in `TargetDetail`'s Overview tab.
#[component]
fn TargetTypeField(
    cid: Signal<String>,
    target_type_id: Option<i64>,
    on_changed: EventHandler<()>,
) -> Element {
    let target_types =
        use_resource(move || async move { api::list_target_types(0, 100, None).await });
    let mut show_dialog = use_signal(|| false);
    let name = match &*target_types.read_unchecked() {
        Some(Ok(page)) => target_type_id
            .and_then(|id| page.content.iter().find(|t| t.id == id))
            .map(|t| t.name.clone()),
        _ => None,
    };
    rsx! {
        span { {name.clone().unwrap_or_else(|| "none".into())} }
        button {
            class: "ml-2 rounded px-2 py-0.5 text-xs text-fg-dim hover:bg-accent",
            r#type: "button",
            onclick: move |_| show_dialog.set(true),
            if name.is_some() { "Change" } else { "Assign" }
        }
        if name.is_some() {
            button {
                class: "ml-1 rounded px-2 py-0.5 text-xs text-err hover:bg-accent",
                r#type: "button",
                onclick: move |_| {
                    spawn(async move {
                        match api::unassign_target_type(&cid()).await {
                            Ok(()) => {
                                toast_ok("target type unassigned");
                                on_changed.call(());
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                "Unassign"
            }
        }
        AssignTargetTypeDialog { open: show_dialog, cid, on_done: move |_| on_changed.call(()) }
    }
}

#[component]
fn AssignTargetTypeDialog(
    open: Signal<bool>,
    cid: Signal<String>,
    on_done: EventHandler<()>,
) -> Element {
    let types = use_resource(move || async move {
        if open() {
            api::list_target_types(0, 100, None).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(|| None::<i64>);
    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[24rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Assign target type" }
            match &*types.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "text-sm text-muted-foreground", "No target types yet." }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for tt in page.content.clone() {
                            li { key: "{tt.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "radio",
                                        name: "tt",
                                        checked: selected() == Some(tt.id),
                                        onchange: move |_| selected.set(Some(tt.id)),
                                    }
                                    span { "{tt.name}" }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
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
                        let (cid, id) = (cid(), selected().unwrap());
                        spawn(async move {
                            match api::assign_target_type(&cid, id).await {
                                Ok(()) => {
                                    toast_ok("target type assigned");
                                    open.set(false);
                                    on_done.call(());
                                }
                                Err(e) => toast_error(e.to_string()),
                            }
                        });
                    },
                    "Assign"
                }
            }
        }
    }
}
