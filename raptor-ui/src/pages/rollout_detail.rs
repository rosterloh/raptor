use crate::components::ui::{Button, ButtonVariant, Card};
use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;

#[component]
pub fn RolloutDetail(id: i64) -> Element {
    let mut rollout = use_resource(move || async move { api::get_rollout(id).await });
    let mut groups = use_resource(move || async move { api::rollout_groups(id, 0, 100).await });
    use_polling(rollout);
    use_polling(groups);

    let mut confirm_delete = use_signal(|| false);
    let mut confirm_stop = use_signal(|| false);
    let nav = use_navigator();

    // Lifecycle transition (start/pause/resume/stop) with toast + refresh.
    let run = move |op: &'static str| {
        spawn(async move {
            let (res, done) = match op {
                "start" => (api::start_rollout(id).await, "started"),
                "pause" => (api::pause_rollout(id).await, "paused"),
                "stop" => (api::stop_rollout(id).await, "stopped"),
                _ => (api::resume_rollout(id).await, "resumed"),
            };
            match res {
                Ok(_) => toast_ok(format!("rollout {done}")),
                Err(e) => toast_error(e.to_string()),
            }
            rollout.restart();
            groups.restart();
        });
    };
    let title = match &*rollout.read_unchecked() {
        Some(Ok(r)) => format!("{} — raptor", r.name),
        _ => format!("Rollout #{id} — raptor"),
    };

    rsx! {
        document::Title { "{title}" }
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
                    // Stop is the escalation from either live state: pause
                    // leaves already-issued updates running on devices.
                    if r.status == "running" || r.status == "paused" {
                        Button {
                            variant: ButtonVariant::Destructive,
                            onclick: move |_| confirm_stop.set(true),
                            "Stop"
                        }
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
                        span { class: "text-sm text-fg-dim", "{r.total_targets} targets" }
                    }
                    div { class: "mb-4 space-y-2",
                        ProgressBar { counts: r.total_targets_per_status, total: r.total_targets }
                        ProgressLegend { counts: r.total_targets_per_status }
                    }
                    dl { class: "space-y-1 text-sm",
                        if let Some(d) = r.description.clone() {
                            Row { k: "Description", v: d }
                        }
                        div { class: "flex gap-2",
                            dt { class: "w-40 shrink-0 text-muted-foreground", "Distribution set" }
                            dd {
                                Link {
                                    to: Route::DsDetail { id: r.distribution_set_id },
                                    class: "text-primary hover:underline",
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
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        ConfirmDialog {
            title: "Stop rollout".to_string(),
            message: "Stop this rollout and cancel the updates it has already sent out? \
                Devices are asked to cancel and report back; the rollout cannot be resumed."
                .to_string(),
            open: confirm_stop,
            on_confirm: move |_| run("stop"),
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
                            nav.push(Route::rollouts());
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
                    rsx! {
                        Card {
                            div { class: "mb-3 flex items-center justify-between",
                                h2 { class: "font-semibold text-foreground", "Groups" }
                                span { class: "text-sm text-fg-dim", "{finished} / {total} groups finished" }
                            }
                            table { class: TABLE,
                                thead {
                                    tr {
                                        th { class: TH, "Group" }
                                        th { class: TH, "Status" }
                                        th { class: TH, "Targets" }
                                        th { class: TH, "Targets by status" }
                                    }
                                }
                                tbody {
                                    for g in page.content.clone() {
                                        tr { key: "{g.id}",
                                            td { class: TD, "{g.name}" }
                                            td { class: TD, StatusBadge { status: g.status.clone() } }
                                            td { class: TD, "{g.total_targets}" }
                                            td { class: TD,
                                                div { class: "w-56 space-y-1",
                                                    ProgressBar {
                                                        counts: g.total_targets_per_status,
                                                        total: g.total_targets,
                                                    }
                                                    ProgressLegend { counts: g.total_targets_per_status }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-muted-foreground", "Loading groups…" } },
            }
        }
    }
}

#[component]
fn Row(k: String, v: String) -> Element {
    rsx! {
        div { class: "flex gap-2",
            dt { class: "w-40 shrink-0 text-muted-foreground", "{k}" }
            dd { class: "break-all", "{v}" }
        }
    }
}
