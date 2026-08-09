use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Actions(filter: String, offset: u64) -> Element {
    let nav = use_navigator();
    let goto = move |filter: String, offset: u64| {
        nav.replace(Route::Actions { filter, offset });
    };

    // A missing `filter` param (a bare `/actions` visit or an old bookmark)
    // falls back to "" via `Default`, which the match below already treats
    // the same as "all".
    let mut actions = use_resource(use_reactive!(|filter, offset| async move {
        let q = match filter.as_str() {
            "pending" => Some("active==true"),
            "finished" => Some("active==false"),
            _ => None,
        };
        api::all_actions(offset, LIMIT, q).await
    }));
    use_polling(actions);
    let select_value = if filter.is_empty() {
        "all".to_string()
    } else {
        filter.clone()
    };
    rsx! {
        document::Title { "Actions — raptor" }
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-foreground", "Actions" }
            select {
                class: "rounded border border-border bg-card px-3 py-1.5 text-sm",
                value: "{select_value}",
                onchange: move |e| goto(e.value(), 0),
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
                                        Link { to: Route::TargetDetail { cid: cid.clone() }, class: "text-primary hover:underline", "{cid}" }
                                    } else {
                                        span { class: "text-muted-foreground", "-" }
                                    }
                                }
                                td { class: TD, "{a.action_type}" }
                                td { class: TD, "{a.status}" }
                                td { class: TD,
                                    div { class: "flex items-center gap-1.5",
                                        StatusBadge { status: a.detail_status.clone() }
                                        if let Some(label) = logic::fetch_stall_label(&a.status, a.deployment_fetch_count) {
                                            span {
                                                class: "rounded border border-pend-border bg-pend-bg px-1.5 py-0.5 font-mono text-[11px] text-pend-fg",
                                                title: "Repeatedly re-downloading the update without ever reporting progress back — check the device's client, not the server.",
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                                td { class: TD, {logic::format_ts(a.last_modified_at)} }
                                td { class: "{TD} space-x-3",
                                    if a.status == "pending" {
                                        if let Some(cid) = a.target.clone() {
                                            button {
                                                class: "text-xs text-err hover:underline",
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
                                    // The only operator path the API supports today for a
                                    // waiting action: releasing it via the target's
                                    // auto-confirm flag.
                                    if a.detail_status == "wait_for_confirmation" {
                                        if let Some(cid) = a.target.clone() {
                                            button {
                                                class: "text-xs text-primary hover:underline",
                                                onclick: move |_| {
                                                    let cid = cid.clone();
                                                    spawn(async move {
                                                        match api::activate_auto_confirm(&cid).await {
                                                            Ok(()) => toast_ok(format!("auto-confirm activated for {cid}")),
                                                            Err(e) => toast_error(e.to_string()),
                                                        }
                                                        actions.restart();
                                                    });
                                                },
                                                "Activate auto-confirm"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Paginator {
                    offset,
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| goto(filter.clone(), o),
                }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| actions.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
    }
}
