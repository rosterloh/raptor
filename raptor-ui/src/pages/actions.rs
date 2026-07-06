use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Actions() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut filter = use_signal(|| "all".to_string());
    let mut actions = use_resource(move || async move {
        let q = match filter().as_str() {
            "pending" => Some("active==true"),
            "finished" => Some("active==false"),
            _ => None,
        };
        api::all_actions(offset(), LIMIT, q).await
    });
    use_polling(actions);
    rsx! {
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-zinc-100", "Actions" }
            select {
                class: "rounded border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-sm",
                value: "{filter}",
                onchange: move |e| {
                    filter.set(e.value());
                    offset.set(0);
                },
                option { value: "all", "All" }
                option { value: "pending", "Running" }
                option { value: "finished", "Finished" }
            }
        }
        match &*actions.read_unchecked() {
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "ID" }
                            th { class: TH, "Target" }
                            th { class: TH, "Type" }
                            th { class: TH, "Status" }
                            th { class: TH, "Detail" }
                            th { class: TH, "Updated" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for a in page.content.clone() {
                            tr { key: "{a.id}",
                                td { class: TD, "#{a.id}" }
                                td { class: TD,
                                    if let Some(cid) = a.target.clone() {
                                        Link { to: Route::TargetDetail { cid: cid.clone() }, class: "text-emerald-400 hover:underline", "{cid}" }
                                    } else {
                                        span { class: "text-zinc-600", "-" }
                                    }
                                }
                                td { class: TD, "{a.action_type}" }
                                td { class: TD, "{a.status}" }
                                td { class: TD, "{a.detail_status}" }
                                td { class: TD, {logic::format_ts(a.last_modified_at)} }
                                td { class: TD,
                                    if a.status == "pending" {
                                        if let Some(cid) = a.target.clone() {
                                            button {
                                                class: "text-xs text-red-400 hover:underline",
                                                onclick: move |_| {
                                                    let cid = cid.clone();
                                                    let aid = a.id;
                                                    spawn(async move {
                                                        match api::cancel_action(&cid, aid, false).await {
                                                            Ok(()) => toast_ok(format!("cancel requested for #{aid}")),
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
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| actions.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
    }
}
