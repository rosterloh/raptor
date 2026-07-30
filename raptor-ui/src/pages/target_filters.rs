use crate::components::ui::{Button, ButtonVariant, Dialog, Input};
use crate::components::*;
use crate::logic::FilterField;
use crate::{api, logic};
use dioxus::prelude::*;
use raptor_api_types::{TargetFilterCreate, TargetFilterRest, TargetFilterUpdate};
use std::collections::BTreeMap;

const LIMIT: u64 = 25;
/// One page of distribution sets is enough to name the auto-assign column and
/// fill the picker; the API has no bulk id→name lookup.
const DS_LIMIT: u64 = 100;
/// Idle time before the filter form asks the API how many targets a query
/// matches, so typing doesn't fire a request per keystroke.
const PREVIEW_DEBOUNCE_MS: u32 = 400;

#[component]
pub fn TargetFilters() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut query = use_signal(String::new);
    let mut filters = use_resource(move || async move {
        let q = logic::fiql_contains(&["name"], &query());
        api::list_target_filters(offset(), LIMIT, q.as_deref()).await
    });
    let sets = use_resource(move || async move { api::list_ds(0, DS_LIMIT, None).await });

    let mut show_form = use_signal(|| false);
    // None = create, Some = edit that filter.
    let mut form_for = use_signal(|| None::<TargetFilterRest>);
    let mut show_assign = use_signal(|| false);
    let mut assign_for = use_signal(|| None::<TargetFilterRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<TargetFilterRest>);

    let ds_names: BTreeMap<i64, String> = match &*sets.read_unchecked() {
        Some(Ok(page)) => page
            .content
            .iter()
            .map(|d| (d.id, format!("{} {}", d.name, d.version)))
            .collect(),
        _ => BTreeMap::new(),
    };

    rsx! {
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-foreground", "Target filters" }
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New filter"
            }
        }
        div { class: "mb-3",
            SearchBox {
                placeholder: "Search name…",
                on_search: move |s| {
                    query.set(s);
                    offset.set(0);
                },
            }
        }
        match &*filters.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground",
                    "No target filters yet. A filter is a saved FIQL query over targets; attach a distribution set to it and matching targets are updated automatically."
                }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Query" }
                            th { class: TH, "Auto-assign" }
                            th { class: TH, "Modified" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for f in page.content.clone() {
                            tr { key: "{f.id}", class: "hover:bg-card",
                                td { class: TD, "{f.name}" }
                                td { class: "{TD} font-mono text-xs break-all", "{f.query}" }
                                td { class: TD,
                                    AutoAssignCell {
                                        label: logic::auto_assign_label(
                                            f.auto_assign_distribution_set,
                                            f.auto_assign_distribution_set
                                                .and_then(|id| ds_names.get(&id))
                                                .map(String::as_str),
                                            f.auto_assign_action_type.as_deref(),
                                        ),
                                    }
                                }
                                td { class: TD, {logic::format_ts(f.last_modified_at)} }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let f = f.clone();
                                                move |_| {
                                                    form_for.set(Some(f.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let f = f.clone();
                                                move |_| {
                                                    assign_for.set(Some(f.clone()));
                                                    show_assign.set(true);
                                                }
                                            },
                                            "Auto-assign…"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let f = f.clone();
                                                move |_| {
                                                    delete_for.set(Some(f.clone()));
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
                Paginator {
                    offset: offset(),
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| offset.set(o),
                }
            },
            Some(Err(e)) => rsx! {
                ErrorPane { message: e.to_string(), on_retry: move |_| filters.restart() }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
        if show_form() {
            FilterFormDialog {
                open: show_form,
                existing: form_for(),
                on_saved: move |_| filters.restart(),
            }
        }
        match (show_assign(), assign_for()) {
            (true, Some(f)) => rsx! {
                AutoAssignDialog {
                    open: show_assign,
                    filter: f,
                    on_done: move |_| filters.restart(),
                }
            },
            _ => rsx! {},
        }
        ConfirmDialog {
            title: "Delete target filter".to_string(),
            message: delete_for()
                .map(|f| format!(
                    "Delete filter \"{}\"? Any auto-assignment stops; updates already assigned to targets are unaffected.",
                    f.name,
                ))
                .unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(f) = delete_for() else { return };
                spawn(async move {
                    match api::delete_target_filter(f.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", f.name));
                            filters.restart();
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

#[component]
fn AutoAssignCell(label: Option<String>) -> Element {
    match label {
        Some(l) => rsx! {
            span { class: "inline-block rounded border px-2 py-0.5 text-xs bg-ok-bg text-ok-fg border-ok-border",
                "{l}"
            }
        },
        None => rsx! {
            span { class: "text-muted-foreground", "none" }
        },
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

/// Create/edit form. FIQL is validated server-side, so the query field shows the
/// API's own message (400) rather than a second grammar implementation here, and
/// a duplicate name (409) lands on the name field.
#[component]
fn FilterFormDialog(
    open: Signal<bool>,
    existing: Option<TargetFilterRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|f| f.id);
    let init_name = existing
        .as_ref()
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let init_query = existing
        .as_ref()
        .map(|f| f.query.clone())
        .unwrap_or_default();

    let mut name = use_signal(|| init_name);
    let mut query = use_signal(|| init_query);
    let mut name_error = use_signal(|| None::<String>);
    let mut query_error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);

    // Live preview of how many targets the query matches. Restarting the
    // resource on every keystroke drops the pending sleep, so only the query the
    // user paused on reaches the API — which doubles as live FIQL feedback.
    let preview = use_resource(move || async move {
        let q = query().trim().to_string();
        if q.is_empty() {
            return None;
        }
        gloo_timers::future::TimeoutFuture::new(PREVIEW_DEBOUNCE_MS).await;
        Some(api::count_targets(&q).await)
    });

    rsx! {
        Dialog { open, class: "w-[28rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let (n, q) = (name().trim().to_string(), query().trim().to_string());
                    if n.is_empty() || q.is_empty() {
                        return;
                    }
                    name_error.set(None);
                    query_error.set(None);
                    saving.set(true);
                    spawn(async move {
                        let saved = match editing_id {
                            Some(id) => {
                                api::update_target_filter(
                                        id,
                                        &TargetFilterUpdate {
                                            name: Some(n),
                                            query: Some(q),
                                        },
                                    )
                                    .await
                            }
                            None => api::create_target_filter(&TargetFilterCreate { name: n, query: q }).await,
                        };
                        saving.set(false);
                        match saved {
                            Ok(f) => {
                                toast_ok(format!("filter {} saved", f.name));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(api::ApiError::Server { status, message }) => {
                                match logic::filter_error_field(status) {
                                    Some(FilterField::Name) => name_error.set(Some(message)),
                                    Some(FilterField::Query) => query_error.set(Some(message)),
                                    None => toast_error(message),
                                }
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_id.is_some() {
                        "Edit target filter"
                    } else {
                        "New target filter"
                    }
                }
                Input {
                    class: "mb-3",
                    placeholder: "Name",
                    required: true,
                    value: "{name}",
                    oninput: move |e: FormEvent| {
                        name.set(e.value());
                        name_error.set(None);
                    },
                }
                FieldError { message: name_error() }
                Input {
                    class: "mb-2 font-mono text-sm",
                    placeholder: "FIQL query, e.g. updateStatus==error",
                    required: true,
                    value: "{query}",
                    oninput: move |e: FormEvent| {
                        query.set(e.value());
                        query_error.set(None);
                    },
                }
                FieldError { message: query_error() }
                match &*preview.read_unchecked() {
                    Some(Some(Ok(total))) => rsx! {
                        p { class: "mb-3 text-xs text-fg-dim", "matches {total} targets right now" }
                    },
                    Some(Some(Err(e))) => rsx! {
                        p { class: "mb-3 text-xs text-pend", "{e.message()}" }
                    },
                    _ => rsx! {
                        p { class: "mb-3 text-xs text-muted-foreground", "Targets matching the query are shown here." }
                    },
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

/// Attaches (or detaches) the distribution set that matching targets get
/// assigned automatically. An incomplete set is rejected by the API with a 400,
/// which is shown inline instead of as a raw failure.
#[component]
fn AutoAssignDialog(
    open: Signal<bool>,
    filter: TargetFilterRest,
    on_done: EventHandler<()>,
) -> Element {
    let sets = use_resource(move || async move { api::list_ds(0, DS_LIMIT, None).await });
    let mut selected = use_signal(|| filter.auto_assign_distribution_set);
    let mut action_type = use_signal(|| {
        filter
            .auto_assign_action_type
            .clone()
            .unwrap_or_else(|| "forced".to_string())
    });
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let filter_id = filter.id;
    let filter_name = filter.name.clone();
    let attached = filter.auto_assign_distribution_set.is_some();

    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-2 text-lg font-semibold text-foreground", "Auto-assign distribution set" }
            p { class: "mb-3 text-sm text-muted-foreground",
                "Targets matching \"{filter_name}\" are assigned this set as soon as they match."
            }
            match &*sets.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "mb-3 text-sm text-muted-foreground", "No distribution sets yet." }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-3 space-y-1",
                        for ds in page.content.clone() {
                            li { key: "{ds.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "radio",
                                        name: "auto-assign-ds",
                                        checked: selected() == Some(ds.id),
                                        onchange: move |_| {
                                            selected.set(Some(ds.id));
                                            error.set(None);
                                        },
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
                Some(Err(e)) => rsx! {
                    p { class: "mb-3 text-sm text-err", "{e}" }
                },
                None => rsx! {
                    p { class: "text-muted-foreground", "Loading…" }
                },
            }
            label { class: "mb-3 flex items-center gap-2 text-sm text-fg-dim",
                "Action type"
                select {
                    class: "rounded border border-border bg-background px-3 py-1 text-sm text-foreground focus:border-ring focus:outline-none",
                    value: "{action_type}",
                    onchange: move |e| action_type.set(e.value()),
                    option { value: "forced", "forced" }
                    option { value: "soft", "soft" }
                }
            }
            FieldError { message: error() }
            div { class: "flex justify-end gap-2",
                button {
                    class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
                if attached {
                    Button {
                        variant: ButtonVariant::Destructive,
                        disabled: busy(),
                        onclick: move |_| {
                            busy.set(true);
                            spawn(async move {
                                let res = api::clear_auto_assign_ds(filter_id).await;
                                busy.set(false);
                                match res {
                                    Ok(()) => {
                                        toast_ok("auto-assign removed");
                                        open.set(false);
                                        on_done.call(());
                                    }
                                    Err(api::ApiError::Server { message, .. }) => error.set(Some(message)),
                                    Err(e) => toast_error(e.to_string()),
                                }
                            });
                        },
                        "Detach"
                    }
                }
                Button {
                    disabled: selected().is_none() || busy(),
                    onclick: move |_| {
                        let Some(ds_id) = selected() else { return };
                        let forced = action_type() == "forced";
                        error.set(None);
                        busy.set(true);
                        spawn(async move {
                            let res = api::set_auto_assign_ds(filter_id, ds_id, forced).await;
                            busy.set(false);
                            match res {
                                Ok(_) => {
                                    toast_ok("auto-assign saved");
                                    open.set(false);
                                    on_done.call(());
                                }
                                Err(api::ApiError::Server { message, .. }) => error.set(Some(message)),
                                Err(e) => toast_error(e.to_string()),
                            }
                        });
                    },
                    "Save"
                }
            }
        }
    }
}
