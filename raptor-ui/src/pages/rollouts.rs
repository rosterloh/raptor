use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Rollouts(query: String, offset: u64) -> Element {
    let nav = use_navigator();
    let goto = move |query: String, offset: u64| {
        nav.replace(Route::Rollouts { query, offset });
    };

    let q = logic::fiql_contains(&["name"], &query);
    let mut rollouts = use_resource(use_reactive!(|q, offset| async move {
        api::list_rollouts(offset, LIMIT, q.as_deref()).await
    }));
    use_polling(rollouts);
    let mut search_key = use_signal(|| 0u32);

    let active = !query.is_empty();
    use_filter_clear(use_reactive!(|active| active), move || {
        goto(String::new(), 0);
        search_key += 1;
    });

    rsx! {
        h1 { class: HEADING, "Rollouts" }
        div { class: "mb-3",
            SearchBox {
                key: "{search_key}",
                placeholder: "Search name…",
                initial: query.clone(),
                on_search: move |s| goto(s, 0),
            }
        }
        match &*rollouts.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No rollouts yet. Create one via the Management API." }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Status" }
                            th { class: TH, "Targets" }
                            th { class: TH, "Progress" }
                            th { class: TH, "Created" }
                        }
                    }
                    tbody {
                        for r in page.content.clone() {
                            tr { key: "{r.id}", class: ROW,
                                td { class: TD,
                                    Link { to: Route::RolloutDetail { id: r.id }, class: LINK_CELL, "{r.name}" }
                                }
                                td { class: TD, StatusBadge { status: r.status.clone() } }
                                td { class: TD, "{r.total_targets}" }
                                td { class: TD,
                                    div { class: "w-44",
                                        ProgressBar {
                                            counts: r.total_targets_per_status,
                                            total: r.total_targets,
                                        }
                                        p { class: "mt-1 text-xs text-muted-foreground",
                                            "{r.total_targets_per_status.finished} of {r.total_targets} finished"
                                        }
                                    }
                                }
                                td { class: TD, {logic::format_ts(r.created_at)} }
                            }
                        }
                    }
                }
                Paginator {
                    offset,
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| goto(query.clone(), o),
                }
            },
            Some(Err(e)) => rsx! {
                ErrorPane { message: e.to_string(), on_retry: move |_| rollouts.restart() }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
    }
}
