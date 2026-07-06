use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

pub const LIMIT: u64 = 25;

#[component]
pub fn Targets() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut query = use_signal(String::new);
    let mut targets = use_resource(move || async move {
        let q = logic::fiql_contains(&["name", "controllerId"], &query());
        api::list_targets(offset(), LIMIT, q.as_deref()).await
    });
    let nav = use_navigator();
    rsx! {
        h1 { class: HEADING, "Targets" }
        div { class: "mb-3",
            SearchBox {
                placeholder: "Search name or controller id…",
                on_search: move |s| {
                    query.set(s);
                    offset.set(0);
                },
            }
        }
        match &*targets.read_unchecked() {
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Controller ID" }
                            th { class: TH, "Status" }
                            th { class: TH, "Last poll" }
                        }
                    }
                    tbody {
                        for t in page.content.clone() {
                            tr {
                                key: "{t.controller_id}",
                                class: ROW,
                                onclick: {
                                    let cid = t.controller_id.clone();
                                    move |_| {
                                        nav.push(Route::TargetDetail { cid: cid.clone() });
                                    }
                                },
                                td { class: TD, "{t.name}" }
                                td { class: "{TD} font-mono text-xs", "{t.controller_id}" }
                                td { class: TD, StatusBadge { status: t.update_status.clone() } }
                                td { class: TD,
                                    {t.last_controller_request_at.map(logic::format_ts).unwrap_or_else(|| "never".into())}
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
                ErrorPane { message: e.to_string(), on_retry: move |_| targets.restart() }
            },
            None => rsx! {
                p { class: "text-zinc-500", "Loading…" }
            },
        }
    }
}
