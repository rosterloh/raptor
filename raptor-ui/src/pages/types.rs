//! Type catalogue admin: software-module, distribution-set, and target
//! types. Three panels behind one tab strip (mirrors `TagKind` in `tags.rs`)
//! since the three resources have different shapes — key vs. no key,
//! maxAssignments, colour — and aren't worth forcing into one generic form.

use crate::api;
use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use dioxus::prelude::*;
use raptor_api_types::{
    DsTypeCreate, DsTypeRest, DsTypeUpdate, SmTypeCreate, SmTypeRest, SmTypeUpdate,
    TargetTypeCreate, TargetTypeRest, TargetTypeUpdate,
};

const LIMIT: u64 = 25;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Sm,
    Ds,
    Target,
}

#[component]
pub fn Types() -> Element {
    let mut tab = use_signal(|| Tab::Sm);
    rsx! {
        document::Title { "Types — raptor" }
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-foreground", "Types" }
        }
        div { class: "mb-4 flex gap-2",
            for (t , label) in [
                (Tab::Sm, "Module types"),
                (Tab::Ds, "Distribution set types"),
                (Tab::Target, "Target types"),
            ] {
                button {
                    key: "{label}",
                    class: if tab() == t { "rounded px-3 py-1.5 text-sm bg-accent text-foreground border-b-2 border-primary" } else { "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent" },
                    onclick: move |_| tab.set(t),
                    "{label}"
                }
            }
        }
        match tab() {
            Tab::Sm => rsx! { SmTypesPanel {} },
            Tab::Ds => rsx! { DsTypesPanel {} },
            Tab::Target => rsx! { TargetTypesPanel {} },
        }
    }
}

/// Strips the `(HTTP nnn)` suffix so a delete-in-use/composition-locked 409
/// reads as an explained state rather than a raw failure.
fn server_message(e: &api::ApiError) -> String {
    e.message()
}

// ---- software module types ----

#[component]
fn SmTypesPanel() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut types =
        use_resource(move || async move { api::list_sm_types(offset(), LIMIT, None).await });
    let mut show_form = use_signal(|| false);
    let mut form_for = use_signal(|| None::<SmTypeRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<SmTypeRest>);

    rsx! {
        div { class: "mb-3 flex justify-end",
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New module type"
            }
        }
        match &*types.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No module types yet." }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Key" }
                            th { class: TH, "Name" }
                            th { class: TH, "Description" }
                            th { class: TH, "Max assignments" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for t in page.content.clone() {
                            tr { key: "{t.id}", class: ROW,
                                td { class: "{TD} font-mono text-xs", "{t.key}" }
                                td { class: TD, "{t.name}" }
                                td { class: "{TD} text-fg-dim", {t.description.clone().unwrap_or_else(|| "-".into())} }
                                td { class: "{TD} tabular-nums text-fg-dim", "{t.max_assignments}" }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    form_for.set(Some(t.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    delete_for.set(Some(t.clone()));
                                                    confirm_delete.set(true);
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Paginator { offset: offset(), limit: LIMIT, total: page.total, on_change: move |o| offset.set(o) }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| types.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        if show_form() {
            SmTypeFormDialog { open: show_form, existing: form_for(), on_saved: move |_| types.restart() }
        }
        ConfirmDialog {
            title: "Delete module type".to_string(),
            message: delete_for().map(|t| format!("Delete module type \"{}\"?", t.name)).unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(t) = delete_for() else { return };
                spawn(async move {
                    match api::delete_sm_type(t.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", t.name));
                            types.restart();
                        }
                        Err(e) => toast_error(server_message(&e)),
                    }
                });
            },
        }
    }
}

#[component]
fn SmTypeFormDialog(
    open: Signal<bool>,
    existing: Option<SmTypeRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|t| t.id);
    let mut key = use_signal(|| existing.as_ref().map(|t| t.key.clone()).unwrap_or_default());
    let mut name = use_signal(|| {
        existing
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default()
    });
    let mut description = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.description.clone())
            .unwrap_or_default()
    });
    let mut max_assignments = use_signal(|| {
        existing
            .as_ref()
            .map(|t| t.max_assignments.to_string())
            .unwrap_or_else(|| "1".into())
    });
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Dialog { open, class: "w-[26rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    error.set(None);
                    let d = description().trim().to_string();
                    let desc = (!d.is_empty()).then_some(d);
                    spawn(async move {
                        let saved = match editing_id {
                            Some(id) => api::update_sm_type(id, &SmTypeUpdate { description: desc }).await,
                            None => {
                                let max = max_assignments().trim().parse::<i32>().ok();
                                api::create_sm_type(&SmTypeCreate {
                                        key: key(),
                                        name: name(),
                                        description: desc,
                                        max_assignments: max,
                                    })
                                    .await
                                    .and_then(|mut v| {
                                        v.pop()
                                            .ok_or_else(|| api::ApiError::Network("empty response".into()))
                                    })
                            }
                        };
                        match saved {
                            Ok(t) => {
                                toast_ok(format!("module type {} saved", t.name));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(e) => error.set(Some(server_message(&e))),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_id.is_some() { "Edit module type" } else { "New module type" }
                }
                if editing_id.is_none() {
                    Input { class: "mb-3 font-mono text-sm", placeholder: "Key", required: true, value: "{key}",
                        oninput: move |e: FormEvent| key.set(e.value()) }
                    Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                        oninput: move |e: FormEvent| name.set(e.value()) }
                    Input {
                        class: "mb-3",
                        r#type: "number",
                        placeholder: "Max assignments per set",
                        value: "{max_assignments}",
                        oninput: move |e: FormEvent| max_assignments.set(e.value()),
                    }
                } else {
                    p { class: "mb-3 text-sm text-fg-dim", "{key} · {name} — key and name are immutable." }
                }
                Input { class: "mb-3", placeholder: "Description (optional)", value: "{description}",
                    oninput: move |e: FormEvent| description.set(e.value()) }
                if let Some(m) = error() {
                    p { class: "mb-3 text-xs text-err", "{m}" }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Save" }
                }
            }
        }
    }
}

// ---- distribution set types ----

#[component]
fn DsTypesPanel() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut types =
        use_resource(move || async move { api::list_ds_types(offset(), LIMIT, None).await });
    let mut show_form = use_signal(|| false);
    let mut form_for = use_signal(|| None::<DsTypeRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<DsTypeRest>);
    let mut show_modules = use_signal(|| false);
    let mut modules_for = use_signal(|| None::<DsTypeRest>);

    rsx! {
        div { class: "mb-3 flex justify-end",
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New distribution set type"
            }
        }
        match &*types.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No distribution set types yet." }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Key" }
                            th { class: TH, "Name" }
                            th { class: TH, "Description" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for t in page.content.clone() {
                            tr { key: "{t.id}", class: ROW,
                                td { class: "{TD} font-mono text-xs", "{t.key}" }
                                td { class: TD, "{t.name}" }
                                td { class: "{TD} text-fg-dim", {t.description.clone().unwrap_or_else(|| "-".into())} }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    modules_for.set(Some(t.clone()));
                                                    show_modules.set(true);
                                                }
                                            },
                                            "Modules…"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    form_for.set(Some(t.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    delete_for.set(Some(t.clone()));
                                                    confirm_delete.set(true);
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Paginator { offset: offset(), limit: LIMIT, total: page.total, on_change: move |o| offset.set(o) }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| types.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        if show_form() {
            DsTypeFormDialog { open: show_form, existing: form_for(), on_saved: move |_| types.restart() }
        }
        if let Some(t) = modules_for() {
            DsTypeModulesDialog { open: show_modules, ds_type_id: t.id, ds_type_name: t.name.clone() }
        }
        ConfirmDialog {
            title: "Delete distribution set type".to_string(),
            message: delete_for().map(|t| format!("Delete distribution set type \"{}\"?", t.name)).unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(t) = delete_for() else { return };
                spawn(async move {
                    match api::delete_ds_type(t.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", t.name));
                            types.restart();
                        }
                        Err(e) => toast_error(server_message(&e)),
                    }
                });
            },
        }
    }
}

#[component]
fn DsTypeFormDialog(
    open: Signal<bool>,
    existing: Option<DsTypeRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|t| t.id);
    let mut key = use_signal(|| existing.as_ref().map(|t| t.key.clone()).unwrap_or_default());
    let mut name = use_signal(|| {
        existing
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default()
    });
    let mut description = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.description.clone())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Dialog { open, class: "w-[26rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    error.set(None);
                    let d = description().trim().to_string();
                    let desc = (!d.is_empty()).then_some(d);
                    spawn(async move {
                        let saved = match editing_id {
                            Some(id) => api::update_ds_type(id, &DsTypeUpdate { description: desc }).await,
                            None => {
                                api::create_ds_type(&DsTypeCreate {
                                        key: key(),
                                        name: name(),
                                        description: desc,
                                        mandatorymodules: vec![],
                                        optionalmodules: vec![],
                                    })
                                    .await
                                    .and_then(|mut v| {
                                        v.pop()
                                            .ok_or_else(|| api::ApiError::Network("empty response".into()))
                                    })
                            }
                        };
                        match saved {
                            Ok(t) => {
                                toast_ok(format!("distribution set type {} saved", t.name));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(e) => error.set(Some(server_message(&e))),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_id.is_some() { "Edit distribution set type" } else { "New distribution set type" }
                }
                if editing_id.is_none() {
                    Input { class: "mb-3 font-mono text-sm", placeholder: "Key", required: true, value: "{key}",
                        oninput: move |e: FormEvent| key.set(e.value()) }
                    Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                        oninput: move |e: FormEvent| name.set(e.value()) }
                    p { class: "mb-3 text-xs text-muted-foreground",
                        "Module composition is set afterwards, from the \"Modules…\" action."
                    }
                } else {
                    p { class: "mb-3 text-sm text-fg-dim", "{key} · {name} — key and name are immutable." }
                }
                Input { class: "mb-3", placeholder: "Description (optional)", value: "{description}",
                    oninput: move |e: FormEvent| description.set(e.value()) }
                if let Some(m) = error() {
                    p { class: "mb-3 text-xs text-err", "{m}" }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Save" }
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Membership {
    None,
    Mandatory,
    Optional,
}

/// Mandatory/optional module-type composition for one DS type. The API
/// upserts a module type's membership on add (switching it between mandatory
/// and optional is one call), and rejects any change with 409 once the type
/// is in use by a set — reflected here as a banner that locks the selects
/// rather than a toast per failed click.
#[component]
fn DsTypeModulesDialog(open: Signal<bool>, ds_type_id: i64, ds_type_name: String) -> Element {
    let mut mandatory =
        use_resource(move || async move { api::ds_type_mandatory(ds_type_id).await });
    let mut optional = use_resource(move || async move { api::ds_type_optional(ds_type_id).await });
    let all = use_resource(move || async move { api::list_sm_types(0, 100, None).await });
    let mut locked = use_signal(|| None::<String>);

    let membership = move |id: i64| -> Membership {
        let in_list = |res: &Resource<api::ApiResult<Vec<SmTypeRest>>>| {
            res.read_unchecked()
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .is_some_and(|v| v.iter().any(|t| t.id == id))
        };
        if in_list(&mandatory) {
            Membership::Mandatory
        } else if in_list(&optional) {
            Membership::Optional
        } else {
            Membership::None
        }
    };

    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-1 text-lg font-semibold text-foreground", "Modules — {ds_type_name}" }
            p { class: "mb-3 text-xs text-muted-foreground",
                "A distribution set of this type is complete once every mandatory module is assigned."
            }
            if let Some(m) = locked() {
                p { class: "mb-3 rounded border border-border-soft bg-accent px-2 py-1.5 text-xs text-fg-dim",
                    "{m}"
                }
            }
            match &*all.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "mb-3 text-sm text-muted-foreground",
                        "No module types defined yet — create one on the Module types tab first."
                    }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for sm in page.content.clone() {
                            li { key: "{sm.id}",
                                div { class: "flex items-center justify-between gap-2 rounded px-2 py-1 text-sm",
                                    span { "{sm.key}" }
                                    select {
                                        class: "rounded border border-border bg-background px-2 py-1 text-xs text-foreground",
                                        disabled: locked().is_some(),
                                        value: match membership(sm.id) {
                                            Membership::None => "none",
                                            Membership::Mandatory => "mandatory",
                                            Membership::Optional => "optional",
                                        },
                                        onchange: move |e| {
                                            let cur = membership(sm.id);
                                            let new = e.value();
                                            spawn(async move {
                                                let res = match (cur, new.as_str()) {
                                                    (_, "mandatory") => api::ds_type_add_mandatory(ds_type_id, sm.id).await,
                                                    (_, "optional") => api::ds_type_add_optional(ds_type_id, sm.id).await,
                                                    (Membership::Mandatory, "none") => api::ds_type_remove_mandatory(ds_type_id, sm.id).await,
                                                    (Membership::Optional, "none") => api::ds_type_remove_optional(ds_type_id, sm.id).await,
                                                    _ => Ok(()),
                                                };
                                                match res {
                                                    Ok(()) => {
                                                        mandatory.restart();
                                                        optional.restart();
                                                    }
                                                    Err(api::ApiError::Server { status: 409, message }) => {
                                                        locked.set(Some(message));
                                                    }
                                                    Err(e) => toast_error(e.to_string()),
                                                }
                                            });
                                        },
                                        option { value: "none", "—" }
                                        option { value: "mandatory", "Mandatory" }
                                        option { value: "optional", "Optional" }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "mb-3 text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
            }
            div { class: "flex justify-end",
                Button { onclick: move |_| open.set(false), "Done" }
            }
        }
    }
}

// ---- target types ----

#[component]
fn TargetTypesPanel() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut types =
        use_resource(move || async move { api::list_target_types(offset(), LIMIT, None).await });
    let mut show_form = use_signal(|| false);
    let mut form_for = use_signal(|| None::<TargetTypeRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<TargetTypeRest>);
    let mut show_compat = use_signal(|| false);
    let mut compat_for = use_signal(|| None::<TargetTypeRest>);

    rsx! {
        div { class: "mb-3 flex justify-end",
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New target type"
            }
        }
        match &*types.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No target types yet." }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Description" }
                            th { class: TH, "Colour" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for t in page.content.clone() {
                            tr { key: "{t.id}", class: ROW,
                                td { class: TD, "{t.name}" }
                                td { class: "{TD} text-fg-dim", {t.description.clone().unwrap_or_else(|| "-".into())} }
                                td { class: TD,
                                    if let Some(c) = t.colour.clone() {
                                        span { class: "inline-flex items-center gap-1.5",
                                            span { class: "size-2.5 rounded-full", style: "background-color:{c}" }
                                            span { class: "font-mono text-xs text-fg-dim", "{c}" }
                                        }
                                    } else {
                                        span { class: "text-muted-foreground", "-" }
                                    }
                                }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    compat_for.set(Some(t.clone()));
                                                    show_compat.set(true);
                                                }
                                            },
                                            "Compatible sets…"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    form_for.set(Some(t.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    delete_for.set(Some(t.clone()));
                                                    confirm_delete.set(true);
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Paginator { offset: offset(), limit: LIMIT, total: page.total, on_change: move |o| offset.set(o) }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| types.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        if show_form() {
            TargetTypeFormDialog { open: show_form, existing: form_for(), on_saved: move |_| types.restart() }
        }
        if let Some(t) = compat_for() {
            TargetTypeCompatDialog { open: show_compat, target_type_id: t.id, target_type_name: t.name.clone() }
        }
        ConfirmDialog {
            title: "Delete target type".to_string(),
            message: delete_for().map(|t| format!("Delete target type \"{}\"?", t.name)).unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(t) = delete_for() else { return };
                spawn(async move {
                    match api::delete_target_type(t.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", t.name));
                            types.restart();
                        }
                        Err(e) => toast_error(server_message(&e)),
                    }
                });
            },
        }
    }
}

#[component]
fn TargetTypeFormDialog(
    open: Signal<bool>,
    existing: Option<TargetTypeRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|t| t.id);
    let mut name = use_signal(|| {
        existing
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default()
    });
    let mut description = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.description.clone())
            .unwrap_or_default()
    });
    let mut colour = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.colour.clone())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);

    rsx! {
        Dialog { open, class: "w-[26rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    error.set(None);
                    let n = name().trim().to_string();
                    if n.is_empty() {
                        return;
                    }
                    let d = description().trim().to_string();
                    let c = colour().trim().to_string();
                    let (desc, col) = ((!d.is_empty()).then_some(d), (!c.is_empty()).then_some(c));
                    spawn(async move {
                        let saved = match editing_id {
                            Some(id) => {
                                api::update_target_type(
                                        id,
                                        &TargetTypeUpdate { name: Some(n), description: desc, colour: col },
                                    )
                                    .await
                            }
                            None => {
                                api::create_target_type(&TargetTypeCreate {
                                        name: n,
                                        description: desc,
                                        colour: col,
                                        compatibledistributionsettypes: vec![],
                                    })
                                    .await
                                    .and_then(|mut v| {
                                        v.pop()
                                            .ok_or_else(|| api::ApiError::Network("empty response".into()))
                                    })
                            }
                        };
                        match saved {
                            Ok(t) => {
                                toast_ok(format!("target type {} saved", t.name));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(e) => error.set(Some(server_message(&e))),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_id.is_some() { "Edit target type" } else { "New target type" }
                }
                Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                    oninput: move |e: FormEvent| name.set(e.value()) }
                Input { class: "mb-3", placeholder: "Description (optional)", value: "{description}",
                    oninput: move |e: FormEvent| description.set(e.value()) }
                Input {
                    class: "mb-3 font-mono text-sm",
                    placeholder: "Colour, e.g. #00ff00 (optional)",
                    value: "{colour}",
                    oninput: move |e: FormEvent| colour.set(e.value()),
                }
                if editing_id.is_none() {
                    p { class: "mb-3 text-xs text-muted-foreground",
                        "Compatible distribution set types are set afterwards, from the \"Compatible sets…\" action."
                    }
                }
                if let Some(m) = error() {
                    p { class: "mb-3 text-xs text-err", "{m}" }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Save" }
                }
            }
        }
    }
}

/// Which distribution-set types a typed target may be assigned — checked on
/// assignment server-side (400 when it isn't), edited here as plain toggles
/// since the API never locks this sub-resource the way DS composition does.
#[component]
fn TargetTypeCompatDialog(
    open: Signal<bool>,
    target_type_id: i64,
    target_type_name: String,
) -> Element {
    let mut compatible =
        use_resource(move || async move { api::target_type_compatible(target_type_id).await });
    let all = use_resource(move || async move { api::list_ds_types(0, 100, None).await });

    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[26rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Compatible sets — {target_type_name}" }
            match &*all.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "mb-3 text-sm text-muted-foreground",
                        "No distribution set types defined yet."
                    }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for ds in page.content.clone() {
                            li { key: "{ds.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "checkbox",
                                        checked: compatible.read_unchecked().as_ref().and_then(|r| r.as_ref().ok()).is_some_and(|v| v.iter().any(|t| t.id == ds.id)),
                                        onchange: move |e: FormEvent| {
                                            let on = e.checked();
                                            spawn(async move {
                                                let res = if on {
                                                    api::target_type_add_compatible(target_type_id, ds.id).await
                                                } else {
                                                    api::target_type_remove_compatible(target_type_id, ds.id).await
                                                };
                                                match res {
                                                    Ok(()) => compatible.restart(),
                                                    Err(e) => toast_error(e.to_string()),
                                                }
                                            });
                                        },
                                    }
                                    span { "{ds.key}" }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "mb-3 text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
            }
            div { class: "flex justify-end",
                Button { onclick: move |_| open.set(false), "Done" }
            }
        }
    }
}
