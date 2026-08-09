//! Global "jump to anything" search: ⌘K / Ctrl-K from anywhere in the console.
//! State lives here, mounted once by `Shell`, rather than in the pages it
//! jumps to or acts on.

use crate::components::FilterClear;
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::{RolloutRest, TargetRest};

/// Same trick as `SearchBox` and the target-filter preview: sleep inside the
/// resource that's keyed on the typed text, so a keystroke restarts it and
/// drops the pending sleep. No debounce helper needed.
const DEBOUNCE_MS: u32 = 300;
const TARGET_LIMIT: u64 = 5;
const ROLLOUT_LIMIT: u64 = 20;

#[derive(Clone)]
enum Item {
    Nav(&'static str, Route),
    Target(Box<TargetRest>),
    PauseRollout(Box<RolloutRest>),
    ClearFilter,
}

fn nav_items() -> Vec<(&'static str, Route)> {
    vec![
        ("Dashboard", Route::Dashboard {}),
        ("Targets", Route::targets()),
        ("Target filters", Route::target_filters()),
        ("Distributions", Route::distributions()),
        ("Modules", Route::modules()),
        ("Rollouts", Route::rollouts()),
        ("Actions", Route::actions()),
        ("Tags", Route::tags()),
        // The only in-app entry point for a deploy is a distribution set's own
        // "Deploy…" button — this jumps to where the fleet picks one, rather
        // than guessing which set and target the operator means.
        ("Deploy…", Route::distributions()),
    ]
}

#[component]
pub fn CommandPalette(open: Signal<bool>) -> Element {
    let mut text = use_signal(String::new);
    let mut selected = use_signal(|| 0usize);
    let nav = use_navigator();
    let filters = use_context::<FilterClear>();

    let targets = use_resource(move || async move {
        let q = text();
        gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
        let term = q.trim();
        if term.is_empty() {
            return Vec::new();
        }
        let fiql = logic::fiql_contains(&["name", "controllerId"], term);
        api::list_targets(0, TARGET_LIMIT, fiql.as_deref())
            .await
            .map(|p| p.content)
            .unwrap_or_default()
    });

    // Fetched once per open rather than kept live — the palette is a quick jump,
    // not a dashboard, and it closes the moment an action runs.
    let running = use_resource(move || async move {
        if !open() {
            return Vec::new();
        }
        api::list_rollouts(0, ROLLOUT_LIMIT, Some("status==running"))
            .await
            .map(|p| p.content)
            .unwrap_or_default()
    });

    let items: Vec<Item> = {
        let needle = text().trim().to_lowercase();
        let matches = |s: &str| needle.is_empty() || s.to_lowercase().contains(&needle);
        let mut out: Vec<Item> = nav_items()
            .into_iter()
            .filter(|(label, _)| matches(label))
            .map(|(label, route)| Item::Nav(label, route))
            .collect();
        out.extend(
            targets
                .read()
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|t| Item::Target(Box::new(t))),
        );
        out.extend(
            running
                .read()
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter(|r| matches(&r.name))
                .map(|r| Item::PauseRollout(Box::new(r))),
        );
        if filters.active() && matches("clear filter") {
            out.push(Item::ClearFilter);
        }
        out
    };

    // Selection can go stale as results load in (fewer items than before) —
    // clamp rather than index out of bounds.
    let sel = selected().min(items.len().saturating_sub(1));

    let mut close = move || {
        open.set(false);
        text.set(String::new());
        selected.set(0);
    };

    let mut activate = move |item: &Item| {
        close();
        match item {
            Item::Nav(_, route) => {
                nav.push(route.clone());
            }
            Item::Target(t) => {
                nav.push(Route::TargetDetail {
                    cid: t.controller_id.clone(),
                });
            }
            Item::PauseRollout(r) => {
                let id = r.id;
                let name = r.name.clone();
                spawn(async move {
                    match api::pause_rollout(id).await {
                        Ok(_) => crate::components::toast_ok(format!("paused {name}")),
                        Err(e) => crate::components::toast_error(e.to_string()),
                    }
                });
            }
            Item::ClearFilter => filters.clear(),
        }
    };

    rsx! {
        if open() {
            div {
                class: "fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-32",
                tabindex: "-1",
                onclick: move |_| close(),
                onkeydown: move |e: KeyboardEvent| {
                    match e.key() {
                        Key::Escape => close(),
                        Key::ArrowDown => {
                            e.prevent_default();
                            if !items.is_empty() {
                                selected.set((sel + 1) % items.len());
                            }
                        }
                        Key::ArrowUp => {
                            e.prevent_default();
                            if !items.is_empty() {
                                selected.set((sel + items.len() - 1) % items.len());
                            }
                        }
                        Key::Enter => {
                            if let Some(item) = items.get(sel) {
                                activate(item);
                            }
                        }
                        _ => {}
                    }
                },
                div {
                    class: "w-full max-w-lg rounded-lg border border-border-soft bg-card shadow-lg",
                    role: "dialog",
                    aria_modal: "true",
                    aria_label: "Command palette",
                    onclick: move |e| e.stop_propagation(),
                    input {
                        class: "w-full border-b border-border-soft bg-transparent px-4 py-3 text-sm outline-none placeholder:text-muted-foreground",
                        placeholder: "Jump to a target, page, or action…",
                        // The `autofocus` attribute only applies to elements present at
                        // initial page load, not ones inserted later by a conditional
                        // render — this dialog only ever appears the second way, so
                        // focus has to be grabbed explicitly once it's mounted.
                        onmounted: move |e| {
                            spawn(async move {
                                let _ = e.set_focus(true).await;
                            });
                        },
                        value: "{text}",
                        oninput: move |e: FormEvent| {
                            text.set(e.value());
                            selected.set(0);
                        },
                    }
                    if items.is_empty() {
                        p { class: "px-4 py-6 text-center text-sm text-muted-foreground", "No matches" }
                    } else {
                        ul { class: "max-h-80 overflow-y-auto py-1", role: "listbox",
                            for (i , item) in items.iter().enumerate() {
                                li {
                                    key: "{i}",
                                    role: "option",
                                    aria_selected: if i == sel { "true" } else { "false" },
                                    class: if i == sel { "cursor-pointer bg-accent px-4 py-2 text-sm text-foreground" } else { "cursor-pointer px-4 py-2 text-sm text-foreground" },
                                    onmouseenter: move |_| selected.set(i),
                                    onclick: {
                                        let item = item.clone();
                                        move |_| activate(&item)
                                    },
                                    {item_label(item)}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn item_label(item: &Item) -> String {
    match item {
        Item::Nav(label, _) => format!("Go to {label}"),
        Item::Target(t) => format!("Target · {} ({})", t.name, t.controller_id),
        Item::PauseRollout(r) => format!("Pause rollout · {}", r.name),
        Item::ClearFilter => "Clear filter".to_string(),
    }
}
