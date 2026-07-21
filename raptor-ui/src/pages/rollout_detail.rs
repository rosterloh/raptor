use crate::components::ui::{Button, ButtonVariant, Card};
use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

#[component]
pub fn RolloutDetail(id: i64) -> Element {
    let mut rollout = use_resource(move || async move { api::get_rollout(id).await });
    let mut groups = use_resource(move || async move { api::rollout_groups(id, 0, 100).await });
    use_polling(rollout);
    use_polling(groups);

    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();

    // Lifecycle transition (start/pause/resume) with toast + refresh.
    let run = move |op: &'static str| {
        spawn(async move {
            let res = match op {
                "start" => api::start_rollout(id).await,
                "pause" => api::pause_rollout(id).await,
                _ => api::resume_rollout(id).await,
            };
            match res {
                Ok(_) => toast_ok(format!("rollout {op}ed")),
                Err(e) => toast_error(e.to_string()),
            }
            rollout.restart();
            groups.restart();
        });
    };

    rsx! {
        match &*rollout.read_unchecked() {
            Some(Ok(r)) => rsx! {
                h1 { class: HEADING, "{r.name}" }
                div { class: "mb-4 flex items-center gap-2",
                    if r.status == "ready" {
                        Button { onclick: move |_| run("start"), "Start" }
                    }
                    if r.status == "running" {
                        Button { onclick: move |_| run("pause"), "Pause" }
                    }
                    if r.status == "paused" {
                        Button { onclick: move |_| run("resume"), "Resume" }
                    }
                    Button {
                        variant: ButtonVariant::Destructive,
                        onclick: move |_| confirm_delete.set(true),
                        "Delete"
                    }
                }
                Card {
                    div { class: "mb-3 flex items-center gap-3",
                        StatusBadge { status: r.status.clone() }
                        span { class: "text-sm text-zinc-400", "{r.total_targets} targets" }
                    }
                    dl { class: "space-y-1 text-sm",
                        if let Some(d) = r.description.clone() {
                            Row { k: "Description", v: d }
                        }
                        div { class: "flex gap-2",
                            dt { class: "w-40 shrink-0 text-zinc-500", "Distribution set" }
                            dd {
                                Link {
                                    to: Route::DsDetail { id: r.distribution_set_id },
                                    class: "text-emerald-400 hover:underline",
                                    "#{r.distribution_set_id}"
                                }
                            }
                        }
                        Row { k: "Target filter", v: r.target_filter_query.clone() }
                        Row { k: "Created", v: logic::format_ts(r.created_at) }
                        Row { k: "Last modified", v: logic::format_ts(r.last_modified_at) }
                    }
                }
                Groups { groups }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| rollout.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
        ConfirmDialog {
            title: "Delete rollout".to_string(),
            message: "Delete this rollout and its groups? This cannot be undone.".to_string(),
            open: confirm_delete,
            on_confirm: move |_| {
                spawn(async move {
                    match api::delete_rollout(id).await {
                        Ok(()) => {
                            toast_ok("rollout deleted");
                            nav.push(Route::Rollouts {});
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

#[component]
fn Groups(
    groups: Resource<
        api::ApiResult<raptor_api_types::PagedList<raptor_api_types::RolloutGroupRest>>,
    >,
) -> Element {
    rsx! {
        div { class: "mt-4",
            match &*groups.read_unchecked() {
                Some(Ok(page)) => {
                    let total = page.content.len();
                    let finished = page.content.iter().filter(|g| g.status == "finished").count();
                    let pct = (finished * 100).checked_div(total).unwrap_or(0);
                    rsx! {
                        Card {
                            div { class: "mb-3 flex items-center justify-between",
                                h2 { class: "font-semibold text-zinc-100", "Groups" }
                                span { class: "text-sm text-zinc-400", "{finished} / {total} finished" }
                            }
                            div { class: "mb-4 h-2 w-full overflow-hidden rounded bg-zinc-800",
                                div {
                                    class: "h-full rounded bg-emerald-500 transition-all",
                                    style: "width: {pct}%",
                                }
                            }
                            table { class: TABLE,
                                thead {
                                    tr {
                                        th { class: TH, "Group" }
                                        th { class: TH, "Status" }
                                        th { class: TH, "Targets" }
                                    }
                                }
                                tbody {
                                    for g in page.content.clone() {
                                        tr { key: "{g.id}",
                                            td { class: TD, "{g.name}" }
                                            td { class: TD, StatusBadge { status: g.status.clone() } }
                                            td { class: TD, "{g.total_targets}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                None => rsx! { p { class: "text-zinc-500", "Loading groups…" } },
            }
        }
    }
}

#[component]
fn Row(k: String, v: String) -> Element {
    rsx! {
        div { class: "flex gap-2",
            dt { class: "w-40 shrink-0 text-zinc-500", "{k}" }
            dd { class: "break-all", "{v}" }
        }
    }
}
