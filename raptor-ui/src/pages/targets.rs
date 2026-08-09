use crate::components::*;
use crate::pages::TagKind;
use crate::{Route, api, logic};
use dioxus::prelude::*;

pub const LIMIT: u64 = 25;

/// The states a target can be in, in the order an operator cares about them:
/// what needs attention first, what is settled last.
const STATES: [(&str, &str, logic::Tone); 4] = [
    ("error", "Error", logic::Tone::Error),
    ("pending", "Pending", logic::Tone::Pending),
    ("registered", "Registered", logic::Tone::Info),
    ("in_sync", "In sync", logic::Tone::Ok),
];

#[component]
pub fn Targets(query: String, state: String, tag: String, offset: u64) -> Element {
    let nav = use_navigator();
    // Filter and pagination state live in the URL (#81) so Back, refresh, and
    // bookmarks all preserve them; navigating with `replace` (not `push`) keeps
    // per-keystroke/per-click changes off the back stack.
    let goto = move |query: String, state: String, tag: String, offset: u64| {
        nav.replace(Route::Targets {
            query,
            state,
            tag,
            offset,
        });
    };

    // The compiled query is shown to the operator, so build it once and reuse it
    // for both the request and the display — otherwise the two could disagree.
    let fiql = logic::fiql_and(&[
        logic::fiql_contains(&["name", "controllerId"], &query),
        (!state.is_empty()).then(|| format!("updateStatus=={state}")),
        logic::fiql_tag(&tag),
    ]);

    let mut targets = use_resource(use_reactive!(|fiql, offset| async move {
        api::list_targets(offset, LIMIT, fiql.as_deref()).await
    }));
    // Polled because the rows carry poll ages: an age that silently stops
    // advancing is worse than no age at all.
    use_polling(targets);

    let tags = use_resource(move || async move {
        api::list_tags(TagKind::Target.prefix(), 0, 100, None).await
    });

    // SearchBox owns its typed text internally, so clearing `query` alone leaves
    // stale text on screen; bumping this and keying the box on it forces a
    // remount, which resets that internal state too.
    let mut search_key = use_signal(|| 0u32);

    let active = !query.is_empty() || !state.is_empty() || !tag.is_empty();
    use_filter_clear(use_reactive!(|active| active), move || {
        goto(String::new(), String::new(), String::new(), 0);
        search_key += 1;
    });

    let now = now_ms();

    rsx! {
        div { class: "mb-5 flex items-end justify-between gap-4",
            div {
                h1 { class: "font-display text-3xl font-bold tracking-wider uppercase text-foreground",
                    "Targets"
                }
                p { class: "mt-0.5 font-mono text-xs text-muted-foreground",
                    match &*targets.read_unchecked() {
                        Some(Ok(page)) => rsx! { "{page.total} matching · {LIMIT} per page" },
                        _ => rsx! { "…" },
                    }
                }
            }
        }

        SectionRule { label: "Filter" }
        div { class: "flex flex-wrap items-center gap-3",
            SearchBox {
                key: "{search_key}",
                placeholder: "name or controller id…",
                initial: query.clone(),
                on_search: {
                    let (state, tag) = (state.clone(), tag.clone());
                    move |s| goto(s, state.clone(), tag.clone(), 0)
                },
            }
            // State is the fleet's primary axis, so it gets chips rather than
            // being buried in a select.
            div { class: "flex flex-wrap gap-2", role: "group", aria_label: "Filter by state",
                Chip {
                    label: "All".to_string(),
                    pressed: state.is_empty(),
                    onclick: {
                        let (query, tag) = (query.clone(), tag.clone());
                        move |_| goto(query.clone(), String::new(), tag.clone(), 0)
                    },
                }
                for (key , label , tone) in STATES {
                    Chip {
                        key: "{key}",
                        label: label.to_string(),
                        tone,
                        pressed: state == key,
                        onclick: {
                            let (query, tag) = (query.clone(), tag.clone());
                            move |_| goto(query.clone(), key.to_string(), tag.clone(), 0)
                        },
                    }
                }
            }
        }

        // Tags are the only way to segment a mixed fleet — raptor's FIQL cannot
        // query the attributes a device reports about itself (#66) — so they get
        // first-class chips too. Filtering by tag compiles to one `tag==` term
        // server-side; the per-row tags in the table below come from the list
        // payload itself (#70), not a request per row.
        match &*tags.read_unchecked() {
            Some(Ok(page)) if !page.content.is_empty() => rsx! {
                div { class: "mt-3 flex flex-wrap gap-2", role: "group", aria_label: "Filter by tag",
                    Chip {
                        label: "All tags".to_string(),
                        pressed: tag.is_empty(),
                        onclick: {
                            let (query, state) = (query.clone(), state.clone());
                            move |_| goto(query.clone(), state.clone(), String::new(), 0)
                        },
                    }
                    for t in page.content.clone() {
                        Chip {
                            key: "{t.id}",
                            label: t.name.clone(),
                            dot: logic::tag_colour(t.colour.as_deref()),
                            pressed: tag == t.name,
                            onclick: {
                                let (query, state, name) = (query.clone(), state.clone(), t.name.clone());
                                move |_| goto(query.clone(), state.clone(), name.clone(), 0)
                            },
                        }
                    }
                }
            },
            _ => rsx! {},
        }

        // The query the filters compile to. Operators live in the Management API
        // as well as the console, so showing it teaches the query language for
        // free and makes a surprising result explainable.
        p { class: "mt-3 font-mono text-[11px] break-all text-muted-foreground",
            match &fiql {
                Some(q) => rsx! { "q={q}" },
                None => rsx! { "no filter — all targets" },
            }
        }

        div { class: "mt-4 border border-border-soft bg-card",
            match &*targets.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    div { class: "p-8 text-center",
                        p { class: "font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                            "No matches"
                        }
                        p { class: "mt-2 text-sm text-muted-foreground",
                            "Nothing matches this filter. Clear the search, or widen the state and tag filters."
                        }
                    }
                },
                Some(Ok(page)) => rsx! {
                    table { class: TABLE,
                        thead {
                            tr {
                                th { class: TH, "Name" }
                                th { class: TH, "Controller ID" }
                                th { class: TH, "Tags" }
                                th { class: TH, "Installed set" }
                                th { class: TH, "State" }
                                th { class: "{TH} text-right", "Last poll" }
                            }
                        }
                        tbody {
                            for t in page.content.clone() {
                                tr { key: "{t.controller_id}", class: ROW,
                                    td { class: TD,
                                        Link {
                                            to: Route::TargetDetail { cid: t.controller_id.clone() },
                                            class: LINK_CELL,
                                            "{t.name}"
                                        }
                                    }
                                    td { class: "{TD} font-mono text-xs text-foreground", "{t.controller_id}" }
                                    td { class: TD,
                                        if t.tags.is_empty() {
                                            span { class: "text-muted-foreground", "—" }
                                        } else {
                                            div { class: "flex flex-wrap gap-1",
                                                for tg in t.tags.clone() {
                                                    TagChip {
                                                        key: "{tg.id}",
                                                        name: tg.name.clone(),
                                                        colour: tg.colour.clone(),
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    // The installed *set*, not a bare version: a
                                    // sensor's 2.1.3 and a gateway's 3.4.0 are
                                    // different upgrade lines. The arrow appears
                                    // only while the device is behind what it has
                                    // been told to run.
                                    td { class: TD,
                                        match (&t.installed_ds, &t.assigned_ds) {
                                            (Some(i), _) => rsx! {
                                                span { class: "block font-mono text-xs text-foreground", "{i.name}" }
                                                span { class: "font-mono text-[11px] text-muted-foreground",
                                                    "{i.version} · {i.ds_type}"
                                                    if t.assigned_ds.as_ref().is_some_and(|a| a.id != i.id) {
                                                        span { class: "text-pend",
                                                            " → {t.assigned_ds.as_ref().unwrap().version}"
                                                        }
                                                    }
                                                }
                                            },
                                            (None, Some(a)) => rsx! {
                                                span { class: "block font-mono text-xs text-muted-foreground", "nothing installed" }
                                                span { class: "font-mono text-[11px] text-pend", "→ {a.name} {a.version}" }
                                            },
                                            (None, None) => rsx! {
                                                span { class: "text-muted-foreground", "—" }
                                            },
                                        }
                                    }
                                    td { class: TD, StatusBadge { status: t.update_status.clone() } }
                                    td {
                                        class: "{TD} text-right font-mono text-xs text-muted-foreground",
                                        title: {t.last_controller_request_at.map(logic::format_ts).unwrap_or_default()},
                                        {logic::relative_age(now, t.last_controller_request_at)}
                                    }
                                }
                            }
                        }
                    }
                    Paginator {
                        offset,
                        limit: LIMIT,
                        total: page.total,
                        on_change: {
                            let (query, state, tag) = (query.clone(), state.clone(), tag.clone());
                            move |o| goto(query.clone(), state.clone(), tag.clone(), o)
                        },
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "p-4",
                        ErrorPane { message: e.to_string(), on_retry: move |_| targets.restart() }
                    }
                },
                None => rsx! {
                    p { class: "p-6 text-sm text-muted-foreground", "Loading…" }
                },
            }
        }
    }
}
