use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use crate::pages::{TagFilter, TagKind};
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::DsCreate;

const LIMIT: u64 = 25;

#[component]
pub fn Distributions(query: String, tag: String, offset: u64) -> Element {
    let nav = use_navigator();
    let goto = move |query: String, tag: String, offset: u64| {
        nav.replace(Route::Distributions { query, tag, offset });
    };

    let q = logic::fiql_and(&[
        logic::fiql_contains(&["name", "version"], &query),
        logic::fiql_tag(&tag),
    ]);
    let mut sets = use_resource(use_reactive!(|q, offset| async move {
        api::list_ds(offset, LIMIT, q.as_deref()).await
    }));
    let mut show_create = use_signal(|| false);
    let mut search_key = use_signal(|| 0u32);

    let active = !query.is_empty() || !tag.is_empty();
    use_filter_clear(use_reactive!(|active| active), move || {
        goto(String::new(), String::new(), 0);
        search_key += 1;
    });

    // TagFilter binds to a `Signal<String>`, so the current tag is mirrored
    // into one; picking a new tag both updates the box and navigates. The
    // effect keeps it in sync when the prop changes from outside the dropdown
    // (back/forward navigation, "Clear filter").
    let mut tag_signal = use_signal(|| tag.clone());
    use_effect(use_reactive!(|tag| tag_signal.set(tag)));
    rsx! {
        document::Title { "Distributions — raptor" }
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-foreground", "Distributions" }
            Button { onclick: move |_| show_create.set(true), "New distribution set" }
        }
        div { class: "mb-3 flex items-center gap-3",
            div { class: "flex-1",
                SearchBox {
                    key: "{search_key}",
                    placeholder: "Search name or version…",
                    initial: query.clone(),
                    on_search: {
                        let tag = tag.clone();
                        move |s| goto(s, tag.clone(), 0)
                    },
                }
            }
            TagFilter {
                kind: TagKind::Ds,
                selected: tag_signal,
                on_change: {
                    let query = query.clone();
                    move |_| goto(query.clone(), tag_signal(), 0)
                },
            }
        }
        match &*sets.read_unchecked() {
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Version" }
                            th { class: TH, "Type" }
                            th { class: TH, "Complete" }
                            th { class: TH, "Valid" }
                            th { class: TH, "Created" }
                        }
                    }
                    tbody {
                        for ds in page.content.clone() {
                            tr { key: "{ds.id}", class: ROW,
                                td { class: TD,
                                    Link { to: Route::DsDetail { id: ds.id }, class: LINK_CELL, "{ds.name}" }
                                }
                                td { class: TD, "{ds.version}" }
                                td { class: TD, "{ds.ds_type}" }
                                td { class: TD, if ds.complete { "yes" } else { "no" } }
                                td { class: TD,
                                    if !ds.valid {
                                        span { class: "inline-block rounded border px-2 py-0.5 text-xs bg-err-bg text-err-fg border-err-border",
                                            "invalid"
                                        }
                                    }
                                }
                                td { class: TD, {logic::format_ts(ds.created_at)} }
                            }
                        }
                    }
                }
                Paginator {
                    offset,
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| goto(query.clone(), tag.clone(), o),
                }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| sets.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        CreateDsDialog { open: show_create, on_created: move |_| sets.restart() }
    }
}

#[component]
fn CreateDsDialog(open: Signal<bool>, on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut version = use_signal(String::new);
    let mut ds_type = use_signal(String::new);
    // Fetched only while open, like AssignDsDialog's set list — this dialog is
    // always mounted, but the type catalogue only matters once it's shown.
    let types = use_resource(move || async move {
        if open() {
            api::list_ds_types(0, 100, None).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    // Default to the first type once the catalogue loads, same as the old
    // hardcoded "os" default.
    use_effect(move || {
        if ds_type().is_empty()
            && let Some(Ok(page)) = &*types.read_unchecked()
            && let Some(first) = page.content.first()
        {
            ds_type.set(first.key.clone());
        }
    });
    rsx! {
        Dialog { open,
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let ds = DsCreate {
                        name: name(),
                        version: version(),
                        ds_type: ds_type(),
                        description: None,
                        required_migration_step: false,
                        modules: vec![],
                    };
                    spawn(async move {
                        match api::create_ds(&ds).await {
                            Ok(_) => {
                                toast_ok("distribution set created");
                                open.set(false);
                                name.set(String::new());
                                version.set(String::new());
                                on_created.call(());
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground", "New distribution set" }
                Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                    oninput: move |e: FormEvent| name.set(e.value()) }
                Input { class: "mb-3", placeholder: "Version", required: true, value: "{version}",
                    oninput: move |e: FormEvent| version.set(e.value()) }
                select {
                    class: SELECT,
                    value: "{ds_type}",
                    onchange: move |e| ds_type.set(e.value()),
                    match &*types.read_unchecked() {
                        Some(Ok(page)) => rsx! {
                            for t in page.content.clone() {
                                option { key: "{t.id}", value: "{t.key}", "{t.key}" }
                            }
                        },
                        _ => rsx! {},
                    }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Create" }
                }
            }
        }
    }
}
