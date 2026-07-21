use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Rollouts() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut query = use_signal(String::new);
    let mut rollouts = use_resource(move || async move {
        let q = logic::fiql_contains(&["name"], &query());
        api::list_rollouts(offset(), LIMIT, q.as_deref()).await
    });
    use_polling(rollouts);
    let nav = use_navigator();
    rsx! {
        h1 { class: HEADING, "Rollouts" }
        div { class: "mb-3",
            SearchBox {
                placeholder: "Search name…",
                on_search: move |s| {
                    query.set(s);
                    offset.set(0);
                },
            }
        }
        match &*rollouts.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-zinc-500", "No rollouts yet. Create one via the Management API." }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Status" }
                            th { class: TH, "Targets" }
                            th { class: TH, "Created" }
                        }
                    }
                    tbody {
                        for r in page.content.clone() {
                            tr {
                                key: "{r.id}",
                                class: ROW,
                                onclick: move |_| {
                                    nav.push(Route::RolloutDetail { id: r.id });
                                },
                                td { class: TD, "{r.name}" }
                                td { class: TD, StatusBadge { status: r.status.clone() } }
                                td { class: TD, "{r.total_targets}" }
                                td { class: TD, {logic::format_ts(r.created_at)} }
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
                ErrorPane { message: e.to_string(), on_retry: move |_| rollouts.restart() }
            },
            None => rsx! {
                p { class: "text-zinc-500", "Loading…" }
            },
        }
    }
}
