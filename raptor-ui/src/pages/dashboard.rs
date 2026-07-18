use crate::components::ui::Card;
use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    let mut data = use_resource(|| async {
        let targets = api::list_targets(0, 500, None).await?;
        let recent = api::all_actions(0, 15, None).await?;
        Ok::<_, api::ApiError>((targets, recent))
    });
    use_polling(data);
    rsx! {
        h1 { class: HEADING, "Dashboard" }
        match &*data.read_unchecked() {
            Some(Ok((targets, recent))) => {
                let count = |s: &str| targets.content.iter().filter(|t| t.update_status == s).count();
                let running = recent.content.iter().filter(|a| a.status == "pending").count();
                rsx! {
                    div { class: "mb-6 grid grid-cols-5 gap-4",
                        Tile { label: "Targets", value: targets.total.to_string(), accent: "text-zinc-100" }
                        Tile { label: "In sync", value: count("in_sync").to_string(), accent: "text-emerald-400" }
                        Tile { label: "Pending", value: count("pending").to_string(), accent: "text-amber-400" }
                        Tile { label: "Error", value: count("error").to_string(), accent: "text-red-400" }
                        Tile { label: "Running actions", value: running.to_string(), accent: "text-sky-400" }
                    }
                    Card {
                        h2 { class: "mb-2 font-semibold text-zinc-100", "Recent actions" }
                        if recent.content.is_empty() {
                            p { class: "text-sm text-zinc-500", "No actions yet." }
                        } else {
                            ul { class: "space-y-2 text-sm",
                                for a in recent.content.clone() {
                                    li { key: "{a.id}", class: "flex justify-between",
                                        span {
                                            "#{a.id} {a.action_type} — {a.detail_status}"
                                            if let Some(cid) = a.target.clone() {
                                                Link { to: Route::TargetDetail { cid: cid.clone() }, class: "ml-2 text-emerald-400 hover:underline", "{cid}" }
                                            }
                                        }
                                        span { class: "text-zinc-500", {logic::format_ts(a.last_modified_at)} }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| data.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
    }
}

#[component]
fn Tile(label: String, value: String, accent: String) -> Element {
    rsx! {
        Card {
            p { class: "text-xs uppercase tracking-wide text-zinc-500", "{label}" }
            p { class: "mt-1 text-2xl font-bold {accent}", "{value}" }
        }
    }
}
