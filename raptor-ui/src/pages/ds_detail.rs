use crate::components::ui::{Button, ButtonVariant, Card, Dialog, Input};
use crate::components::*;
use crate::pages::{EntityTags, TagKind};
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::DsUpdate;

#[component]
pub fn DsDetail(id: i64) -> Element {
    let mut ds = use_resource(move || async move { api::get_ds(id).await });
    let tags = use_resource(move || async move { api::ds_tags(id).await });
    let mut show_modules = use_signal(|| false);
    let mut show_deploy = use_signal(|| false);
    let mut show_edit = use_signal(|| false);
    let mut show_invalidate = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();
    let current = ds
        .read_unchecked()
        .as_ref()
        .and_then(|r| r.as_ref().ok().cloned());
    rsx! {
        match &*ds.read_unchecked() {
            Some(Ok(d)) => rsx! {
                div { class: "mb-4 flex items-center gap-2",
                    h1 { class: "text-xl font-bold text-foreground", "{d.name} {d.version}" }
                    if !d.valid {
                        span { class: "inline-block rounded border px-2 py-0.5 text-xs bg-err-bg text-err-fg border-err-border",
                            "invalid"
                        }
                    }
                }
                div { class: "mb-4 flex gap-2",
                    Button { disabled: !d.valid, onclick: move |_| show_deploy.set(true), "Deploy…" }
                    Button { disabled: !d.valid, onclick: move |_| show_modules.set(true), "Assign modules" }
                    Button { onclick: move |_| show_edit.set(true), "Edit" }
                    if d.valid {
                        Button {
                            variant: ButtonVariant::Destructive,
                            onclick: move |_| show_invalidate.set(true),
                            "Invalidate"
                        }
                    }
                    Button {
                        variant: ButtonVariant::Destructive,
                        onclick: move |_| confirm_delete.set(true),
                        "Delete"
                    }
                }
                Card {
                    p { class: "mb-2 text-sm text-fg-dim",
                        "type {d.ds_type} · "
                        if d.complete { "complete" } else { "incomplete" }
                        " · created {logic::format_ts(d.created_at)}"
                    }
                    h2 { class: "mb-2 font-semibold text-foreground", "Modules" }
                    if d.modules.is_empty() {
                        p { class: "text-sm text-muted-foreground", "No modules assigned — the set is not deployable yet." }
                    } else {
                        ul { class: "space-y-1 text-sm",
                            for m in d.modules.clone() {
                                li { key: "{m.id}",
                                    Link { to: Route::ModuleDetail { id: m.id }, class: "text-primary hover:underline",
                                        "{m.name} {m.version} ({m.module_type})"
                                    }
                                }
                            }
                        }
                    }
                }
                Card { class: "mt-4",
                    EntityTags { kind: TagKind::Ds, entity_key: id.to_string(), tags }
                }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| ds.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        AssignModulesDialog { open: show_modules, ds_id: id, on_done: move |_| ds.restart() }
        DeployDialog { open: show_deploy, ds_id: id }
        if let Some(d) = current.clone() {
            EditDsDialog { open: show_edit, ds: d, on_saved: move |_| ds.restart() }
        }
        InvalidateDsDialog { open: show_invalidate, ds_id: id, on_done: move |_| ds.restart() }
        ConfirmDialog {
            title: "Delete distribution set".to_string(),
            message: "Delete this distribution set? Sets referenced by actions cannot be deleted.".to_string(),
            open: confirm_delete,
            on_confirm: move |_| {
                spawn(async move {
                    match api::delete_ds(id).await {
                        Ok(()) => {
                            toast_ok("deleted");
                            nav.push(Route::distributions());
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

#[component]
fn AssignModulesDialog(open: Signal<bool>, ds_id: i64, on_done: EventHandler<()>) -> Element {
    let modules = use_resource(move || async move {
        if open() {
            api::list_modules(0, 100, None).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(Vec::<i64>::new);
    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Assign software modules" }
            match &*modules.read_unchecked() {
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-4 space-y-1",
                        for m in page.content.clone() {
                            li { key: "{m.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "checkbox",
                                        checked: selected().contains(&m.id),
                                        onchange: move |e| {
                                            let mut s = selected();
                                            if e.checked() { s.push(m.id) } else { s.retain(|&x| x != m.id) }
                                            selected.set(s);
                                        },
                                    }
                                    span { "{m.name} {m.version} ({m.module_type})" }
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
                    disabled: selected().is_empty(),
                    onclick: move |_| {
                        let ids = selected();
                        spawn(async move {
                            match api::ds_assign_modules(ds_id, &ids).await {
                                Ok(()) => {
                                    toast_ok("modules assigned");
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

#[component]
fn DeployDialog(open: Signal<bool>, ds_id: i64) -> Element {
    let mut query = use_signal(String::new);
    let targets = use_resource(move || async move {
        if open() {
            let q = crate::logic::fiql_contains(&["name", "controllerId"], &query());
            api::list_targets(0, 50, q.as_deref()).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut forced = use_signal(|| true);
    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Deploy to target" }
            div { class: "mb-3",
                SearchBox { placeholder: "Search targets…", on_search: move |s| query.set(s) }
            }
            match &*targets.read_unchecked() {
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for t in page.content.clone() {
                            li { key: "{t.controller_id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "radio",
                                        name: "deploy-target",
                                        checked: selected() == Some(t.controller_id.clone()),
                                        onchange: {
                                            let cid = t.controller_id.clone();
                                            move |_| selected.set(Some(cid.clone()))
                                        },
                                    }
                                    span { "{t.name} " }
                                    StatusBadge { status: t.update_status.clone() }
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
                "Forced"
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
                        let (cid, is_forced) = (selected().unwrap(), forced());
                        spawn(async move {
                            match api::assign_ds(&cid, ds_id, is_forced).await {
                                Ok(r) if r.assigned > 0 => toast_ok(format!("deploying to {cid}")),
                                Ok(_) => toast_ok("already assigned"),
                                Err(e) => {
                                    toast_error(e.to_string());
                                    return;
                                }
                            }
                            open.set(false);
                        });
                    },
                    "Deploy"
                }
            }
        }
    }
}

#[component]
fn FieldError(message: Option<String>) -> Element {
    match message {
        Some(m) => rsx! {
            p { class: "mb-3 text-xs text-err", "{m}" }
        },
        None => rsx! {},
    }
}

/// A duplicate (name, version) lands as a 409 on both fields, since the API
/// doesn't say which one collided.
#[component]
fn EditDsDialog(
    open: Signal<bool>,
    ds: raptor_api_types::DsRest,
    on_saved: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| ds.name.clone());
    let mut version = use_signal(|| ds.version.clone());
    let mut description = use_signal(|| ds.description.clone().unwrap_or_default());
    let mut required_migration_step = use_signal(|| ds.required_migration_step);
    let mut error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);
    let ds_id = ds.id;
    rsx! {
        Dialog { open,
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let (n, v) = (name().trim().to_string(), version().trim().to_string());
                    if n.is_empty() || v.is_empty() {
                        return;
                    }
                    error.set(None);
                    saving.set(true);
                    let desc = description();
                    let update = DsUpdate {
                        name: Some(n),
                        version: Some(v),
                        description: Some(desc).filter(|d| !d.is_empty()),
                        required_migration_step: Some(required_migration_step()),
                    };
                    spawn(async move {
                        let res = api::update_ds(ds_id, &update).await;
                        saving.set(false);
                        match res {
                            Ok(_) => {
                                toast_ok("distribution set updated");
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(api::ApiError::Server { message, .. }) => error.set(Some(message)),
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground", "Edit distribution set" }
                Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                    oninput: move |e: FormEvent| name.set(e.value()) }
                Input { class: "mb-3", placeholder: "Version", required: true, value: "{version}",
                    oninput: move |e: FormEvent| version.set(e.value()) }
                FieldError { message: error() }
                Input { class: "mb-3", placeholder: "Description", value: "{description}",
                    oninput: move |e: FormEvent| description.set(e.value()) }
                label { class: "mb-4 flex items-center gap-2 text-sm",
                    input {
                        r#type: "checkbox",
                        checked: required_migration_step(),
                        onchange: move |e| required_migration_step.set(e.checked()),
                    }
                    "Requires migration step"
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", disabled: saving(), "Save" }
                }
            }
        }
    }
}

/// Confirm-and-configure dialog: invalidating stops rollouts and cancels
/// in-flight actions, so it needs the same explicit confirmation as delete,
/// plus the cancelation-type/rollout choices [`ConfirmDialog`] has no room for.
#[component]
fn InvalidateDsDialog(open: Signal<bool>, ds_id: i64, on_done: EventHandler<()>) -> Element {
    let mut action_type = use_signal(|| "none".to_string());
    let mut cancel_rollouts = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    rsx! {
        Dialog { open,
            h3 { class: "mb-2 text-lg font-semibold text-foreground", "Invalidate distribution set" }
            p { class: "mb-4 text-sm text-fg-dim",
                "Invalidating stops the set from being assigned again. In-flight actions and rollouts are only affected if chosen below."
            }
            label { class: "mb-3 flex items-center gap-2 text-sm text-fg-dim",
                "Cancel in-flight actions"
                select {
                    class: SELECT,
                    value: "{action_type}",
                    onchange: move |e| action_type.set(e.value()),
                    option { value: "none", "none" }
                    option { value: "soft", "soft" }
                    option { value: "force", "force" }
                }
            }
            label { class: "mb-4 flex items-center gap-2 text-sm",
                input {
                    r#type: "checkbox",
                    checked: cancel_rollouts(),
                    onchange: move |e| cancel_rollouts.set(e.checked()),
                }
                "Also stop rollouts deploying this set"
            }
            FieldError { message: error() }
            div { class: "flex justify-end gap-2",
                button {
                    class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
                Button {
                    variant: ButtonVariant::Destructive,
                    disabled: busy(),
                    onclick: move |_| {
                        error.set(None);
                        busy.set(true);
                        let body = raptor_api_types::DsInvalidate {
                            action_cancelation_type: Some(action_type()),
                            cancel_rollouts: cancel_rollouts(),
                        };
                        spawn(async move {
                            let res = api::invalidate_ds(ds_id, &body).await;
                            busy.set(false);
                            match res {
                                Ok(()) => {
                                    toast_ok("distribution set invalidated");
                                    open.set(false);
                                    on_done.call(());
                                }
                                Err(api::ApiError::Server { message, .. }) => error.set(Some(message)),
                                Err(e) => toast_error(e.to_string()),
                            }
                        });
                    },
                    "Invalidate"
                }
            }
        }
    }
}
