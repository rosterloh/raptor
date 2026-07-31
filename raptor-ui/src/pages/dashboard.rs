use crate::components::ui::Card;
use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::TenantConfigValue;
use std::collections::BTreeMap;

#[component]
pub fn Dashboard() -> Element {
    let mut data = use_resource(|| async {
        let stats = api::system_statistics().await?;
        let recent = api::all_actions(0, 15, None).await?;
        let rollouts = api::list_rollouts(0, 50, None).await?;
        Ok::<_, api::ApiError>((stats, recent, rollouts))
    });
    use_polling(data);
    // Config is file-driven and can't change under a running server, so this
    // one is read once rather than polled with the rest.
    let configs = use_resource(|| async { api::system_configs().await });
    rsx! {
        h1 { class: HEADING, "Dashboard" }
        match &*data.read_unchecked() {
            Some(Ok((stats, recent, rollouts))) => {
                let count = |s: &str| stats.targets_by_status.get(s).copied().unwrap_or(0);
                let active_rollouts: Vec<_> = rollouts
                    .content
                    .iter()
                    .filter(|r| r.status != "finished")
                    .cloned()
                    .collect();
                rsx! {
                    div { class: "mb-6 grid grid-cols-5 gap-4",
                        Tile { label: "Targets", value: stats.total_targets.to_string(), accent: "text-foreground" }
                        Tile { label: "In sync", value: count("in_sync").to_string(), accent: "text-ok" }
                        Tile { label: "Pending", value: count("pending").to_string(), accent: "text-pend" }
                        Tile { label: "Error", value: count("error").to_string(), accent: "text-err" }
                        Tile { label: "Running actions", value: stats.active_actions.to_string(), accent: "text-info" }
                    }
                    if !active_rollouts.is_empty() {
                        Card { class: "mb-6",
                            h2 { class: "mb-2 font-semibold text-foreground", "Active rollouts" }
                            ul { class: "space-y-2 text-sm",
                                for r in active_rollouts.clone() {
                                    li { key: "{r.id}", class: "space-y-1",
                                        div { class: "flex items-center justify-between",
                                            Link {
                                                to: Route::RolloutDetail { id: r.id },
                                                class: "text-primary hover:underline",
                                                "{r.name}"
                                            }
                                            span { class: "flex items-center gap-2 text-muted-foreground",
                                                "{r.total_targets_per_status.finished} / {r.total_targets} targets finished"
                                                StatusBadge { status: r.status.clone() }
                                            }
                                        }
                                        ProgressBar {
                                            counts: r.total_targets_per_status,
                                            total: r.total_targets,
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Card { class: "mb-6",
                        h2 { class: "mb-2 font-semibold text-foreground", "Recent actions" }
                        if recent.content.is_empty() {
                            p { class: "text-sm text-muted-foreground", "No actions yet." }
                        } else {
                            ul { class: "space-y-2 text-sm",
                                for a in recent.content.clone() {
                                    li { key: "{a.id}", class: "flex justify-between",
                                        span {
                                            "#{a.id} {a.action_type} — {a.detail_status}"
                                            if let Some(cid) = a.target.clone() {
                                                Link { to: Route::TargetDetail { cid: cid.clone() }, class: "ml-2 text-primary hover:underline", "{cid}" }
                                            }
                                        }
                                        span { class: "text-muted-foreground", {logic::format_ts(a.last_modified_at)} }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| data.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        SystemCard { configs }
    }
}

/// The server's tenant configuration as the devices see it. Purely
/// informational: raptor takes its config from the file, and the API answers
/// writes with 403, so there is nothing to edit here.
#[component]
fn SystemCard(configs: Resource<api::ApiResult<BTreeMap<String, TenantConfigValue>>>) -> Element {
    rsx! {
        Card {
            div { class: "mb-2 flex items-center justify-between",
                h2 { class: "font-semibold text-foreground", "System configuration" }
                span { class: "text-xs text-muted-foreground", "read-only — set in raptor.toml" }
            }
            match &*configs.read_unchecked() {
                Some(Ok(cfg)) => {
                    let keys: Vec<&str> = cfg.keys().map(String::as_str).collect();
                    rsx! {
                        dl { class: "grid grid-cols-2 gap-4 text-sm",
                            for k in logic::ordered_config_keys(&keys) {
                                div { key: "{k}", class: "flex gap-2",
                                    dt { class: "w-40 shrink-0 text-muted-foreground", {logic::config_label(&k)} }
                                    dd { class: "break-all text-fg-dim",
                                        {cfg.get(&k).map(|c| logic::format_config_value(&c.value)).unwrap_or_default()}
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-sm text-err", "{e}" } },
                None => rsx! { p { class: "text-sm text-muted-foreground", "Loading…" } },
            }
        }
    }
}

#[component]
fn Tile(label: String, value: String, accent: String) -> Element {
    rsx! {
        Card {
            p { class: "text-xs uppercase tracking-wide text-muted-foreground", "{label}" }
            p { class: "mt-1 text-2xl font-bold {accent}", "{value}" }
        }
    }
}
