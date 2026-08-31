use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Rollouts(query: String, sort: String, offset: u64) -> Element {
    let nav = use_navigator();
    let goto = move |query: String, sort: String, offset: u64| {
        nav.replace(Route::Rollouts {
            query,
            sort,
            offset,
        });
    };

    let q = logic::fiql_contains(&["name"], &query);
    let mut rollouts = use_resource(use_reactive!(|q, offset| async move {
        api::list_rollouts(offset, LIMIT, q.as_deref()).await
    }));
    use_polling(rollouts);
    let mut search_key = use_signal(|| 0u32);

    let active = !query.is_empty();
    use_filter_clear(use_reactive!(|active| active), move || {
        goto(String::new(), String::new(), 0);
        search_key += 1;
    });

    rsx! {
        document::Title { "Rollouts — raptor" }
        h1 { class: HEADING, "Rollouts" }
        div { class: "mb-3",
            SearchBox {
                key: "{search_key}",
                placeholder: "Search name…",
                initial: query.clone(),
                on_search: {
                    let sort = sort.clone();
                    move |s| goto(s, sort.clone(), 0)
                },
            }
        }
        match &*rollouts.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No rollouts yet. Create one via the Management API." }
            },
            Some(Ok(page)) => {
                let mut rows = page.content.clone();
                match sort.trim_start_matches('-') {
                    "status" => rows.sort_by(|a, b| a.status.cmp(&b.status)),
                    "progress" => rows.sort_by_key(|r| r.total_targets_per_status.finished),
                    "created" => rows.sort_by_key(|r| r.created_at),
                    _ => {}
                }
                if sort.starts_with('-') { rows.reverse(); }
                let status_mark = logic::sort_mark(&sort, "status");
                let progress_mark = logic::sort_mark(&sort, "progress");
                let created_mark = logic::sort_mark(&sort, "created");
                let (pager_query, pager_sort) = (query.clone(), sort.clone());
                rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH,
                                button { onclick: {
                                    let (query, sort) = (query.clone(), sort.clone());
                                    move |_| goto(query.clone(), logic::next_sort(&sort, "status"), 0)
                                }, "Status{status_mark}" }
                            }
                            th { class: TH, "Targets" }
                            th { class: TH,
                                button { onclick: {
                                    let (query, sort) = (query.clone(), sort.clone());
                                    move |_| goto(query.clone(), logic::next_sort(&sort, "progress"), 0)
                                }, "Progress{progress_mark}" }
                            }
                            th { class: TH,
                                button { onclick: {
                                    let (query, sort) = (query.clone(), sort.clone());
                                    move |_| goto(query.clone(), logic::next_sort(&sort, "created"), 0)
                                }, "Created{created_mark}" }
                            }
                        }
                    }
                    tbody {
                        for r in rows {
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
                    on_change: move |o| goto(pager_query.clone(), pager_sort.clone(), o),
                }
            }},
            Some(Err(e)) => rsx! {
                ErrorPane { message: e.to_string(), on_retry: move |_| rollouts.restart() }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
    }
}
