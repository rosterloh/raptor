use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;

const LIMIT: u64 = 25;

#[component]
pub fn Actions(filter: String, sort: String, offset: u64) -> Element {
    let nav = use_navigator();
    let mut cancel_open = use_signal(|| false);
    let mut cancel_target = use_signal(String::new);
    let mut cancel_id = use_signal(|| 0i64);
    let mut auto_confirm_open = use_signal(|| false);
    let mut auto_confirm_target = use_signal(String::new);
    let goto = move |filter: String, sort: String, offset: u64| {
        nav.replace(Route::Actions {
            filter,
            sort,
            offset,
        });
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
        div { class: "mb-5 flex items-end justify-between gap-4",
            div {
                h1 { class: "font-display text-3xl font-bold tracking-wider text-foreground uppercase", "Actions" }
                p { class: "mt-0.5 font-mono text-xs text-muted-foreground",
                    match &*actions.read_unchecked() {
                        Some(Ok(page)) => rsx! { "{page.total} actions" },
                        _ => rsx! { "…" },
                    }
                }
            }
            select {
                class: "rounded border border-border bg-card px-3 py-1.5 text-sm",
                value: "{select_value}",
                onchange: {
                    let sort = sort.clone();
                    move |e| goto(e.value(), sort.clone(), 0)
                },
                option { value: "all", "All" }
                option { value: "pending", "Running" }
                option { value: "finished", "Finished" }
            }
        }
        match &*actions.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                div { class: "border border-border-soft bg-card p-8 text-center",
                    p { class: "text-sm text-muted-foreground", "No actions match this status." }
                }
            },
            Some(Ok(page)) => {
                let mut rows = page.content.clone();
                match sort.trim_start_matches('-') {
                    "status" => rows.sort_by(|a, b| a.status.cmp(&b.status)),
                    "updated" => rows.sort_by_key(|a| a.last_modified_at),
                    _ => {}
                }
                if sort.starts_with('-') { rows.reverse(); }
                let status_mark = logic::sort_mark(&sort, "status");
                let updated_mark = logic::sort_mark(&sort, "updated");
                let (pager_filter, pager_sort) = (filter.clone(), sort.clone());
                rsx! {
                div { class: "overflow-x-auto border border-border-soft bg-card",
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "ID" }
                            th { class: TH, "Target" }
                            th { class: TH, "Type" }
                            th { class: TH,
                                button { onclick: {
                                    let filter = filter.clone();
                                    let sort = sort.clone();
                                    move |_| goto(filter.clone(), logic::next_sort(&sort, "status"), 0)
                                }, "Status{status_mark}" }
                            }
                            th { class: TH, "Detail" }
                            th { class: TH,
                                button { onclick: {
                                    let filter = filter.clone();
                                    let sort = sort.clone();
                                    move |_| goto(filter.clone(), logic::next_sort(&sort, "updated"), 0)
                                }, "Updated{updated_mark}" }
                            }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for a in rows {
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
                                                    cancel_target.set(cid.clone());
                                                    cancel_id.set(a.id);
                                                    cancel_open.set(true);
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
                                                    auto_confirm_target.set(cid.clone());
                                                    auto_confirm_open.set(true);
                                                },
                                                "Activate auto-confirm"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }}
                Paginator {
                    offset,
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| goto(pager_filter.clone(), pager_sort.clone(), o),
                }
            }},
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| actions.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        ConfirmDialog {
            title: "Cancel action".to_string(),
            message: cancel_action_message(&cancel_target(), cancel_id()),
            open: cancel_open,
            on_confirm: move |_| {
                let (cid, aid) = (cancel_target(), cancel_id());
                spawn(async move {
                    match api::cancel_action(&cid, aid, false).await {
                        Ok(()) => toast_ok(format!("cancel requested for #{aid}")),
                        Err(e) => toast_error(e.to_string()),
                    }
                    actions.restart();
                });
            },
        }
        ConfirmDialog {
            title: "Activate auto-confirm".to_string(),
            message: auto_confirm_message(&auto_confirm_target()),
            open: auto_confirm_open,
            on_confirm: move |_| {
                let cid = auto_confirm_target();
                spawn(async move {
                    match api::activate_auto_confirm(&cid).await {
                        Ok(()) => toast_ok(format!("auto-confirm activated for {cid}")),
                        Err(e) => toast_error(e.to_string()),
                    }
                    actions.restart();
                });
            },
        }
    }
}

fn cancel_action_message(cid: &str, aid: i64) -> String {
    format!("Cancel action #{aid} for {cid}? The device will be asked to stop the update.")
}

fn auto_confirm_message(cid: &str) -> String {
    format!(
        "Activate auto-confirm for {cid}? This changes the target's confirmation behaviour for future assignments too."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_copy_names_the_affected_target_and_action() {
        assert_eq!(
            cancel_action_message("sensor-7", 42),
            "Cancel action #42 for sensor-7? The device will be asked to stop the update."
        );
        assert_eq!(
            auto_confirm_message("sensor-7"),
            "Activate auto-confirm for sensor-7? This changes the target's confirmation behaviour for future assignments too."
        );
    }
}
