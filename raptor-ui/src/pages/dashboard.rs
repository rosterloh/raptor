use crate::components::ui::Card;
use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::{SystemStatistics, TargetFilterRest, TenantConfigValue};
use std::collections::BTreeMap;

/// How many saved filters the segment panel will summarise. Each one costs a
/// scoped statistics request, so this is a real bound rather than a round number.
const MAX_SEGMENTS: u64 = 8;
/// Enough error targets to sort by staleness and still show the worst few.
const ERROR_SCAN: u64 = 100;
const ATTENTION_ROWS: usize = 6;
/// Saved filters and their auto-assigned sets are edited by hand, so they do not
/// need the 5s beat the live counters run on.
const SEGMENT_POLL_MS: u32 = 60_000;

/// What the segment panel needs, fetched together.
#[derive(Clone, PartialEq)]
struct Segments {
    /// Each saved filter paired with its scoped counters — paired rather than
    /// two parallel `Vec`s so they cannot drift out of alignment. `None` counters
    /// mean that filter's query no longer compiles server-side.
    rows: Vec<(TargetFilterRest, Option<SystemStatistics>)>,
    /// Distribution-set names, for resolving the auto-assign column's id.
    ds_names: BTreeMap<i64, String>,
}

#[component]
pub fn Dashboard() -> Element {
    // Live half: counters, in-flight work, and what is failing right now.
    let mut data = use_resource(|| async {
        let stats = api::system_statistics().await?;
        let recent = api::all_actions(0, 10, None).await?;
        let rollouts = api::list_rollouts(0, 50, None).await?;
        // Errors are fetched then sorted here rather than server-side: the
        // Management API has no "order by staleness", and the useful question is
        // which device has been failing longest, not which failed most recently.
        let failing = api::list_targets(0, ERROR_SCAN, Some("updateStatus==error")).await?;
        Ok::<_, api::ApiError>((stats, recent, rollouts, failing))
    });
    use_polling(data);

    // Slow half: the fleet's segments. One scoped statistics call per saved
    // filter — possible at all because of the `q=` parameter on
    // /rest/v1/system/statistics. Without it this panel would cost one request
    // per status per filter, on every tick.
    let segments = use_resource(|| async {
        let filters = api::list_target_filters(0, MAX_SEGMENTS, None).await?;
        let sets = api::list_ds(0, 200, None).await?;
        let stats = futures::future::join_all(
            filters
                .content
                .iter()
                .map(|f| async { api::system_statistics_for(&f.query).await.ok() }),
        )
        .await;
        Ok::<_, api::ApiError>(Segments {
            rows: filters.content.into_iter().zip(stats).collect(),
            ds_names: sets
                .content
                .iter()
                .map(|d| (d.id, d.name.clone()))
                .collect(),
        })
    });
    use_polling_every(segments, SEGMENT_POLL_MS);

    // Config is file-driven and can't change under a running server, so this
    // one is read once rather than polled with the rest.
    let configs = use_resource(|| async { api::system_configs().await });

    // Read once per render rather than per row, so every age on the page is
    // measured against the same instant.
    let now = now_ms();

    rsx! {
        match &*data.read_unchecked() {
            Some(Ok((stats, recent, rollouts, failing))) => {
                let count = |s: &str| stats.targets_by_status.get(s).copied().unwrap_or(0);
                let active_rollouts: Vec<_> = rollouts
                    .content
                    .iter()
                    .filter(|r| r.status != "finished")
                    .cloned()
                    .collect();
                // Longest-stalled first. A target that has never polled sorts to
                // the top, because "never seen" is worse than "late".
                let mut worst = failing.content.clone();
                worst.sort_by_key(|t| t.last_controller_request_at.unwrap_or(i64::MIN));
                worst.truncate(ATTENTION_ROWS);
                rsx! {
                    div { class: "mb-5 flex items-end justify-between gap-4",
                        div {
                            h1 { class: "font-display text-3xl font-bold tracking-wider uppercase text-foreground",
                                "Fleet overview"
                            }
                            p { class: "mt-0.5 font-mono text-xs text-muted-foreground",
                                "{stats.total_targets} registered targets · {stats.active_actions} actions in flight"
                            }
                        }
                        Link {
                            to: Route::targets(),
                            class: "text-sm text-fg-dim underline underline-offset-4 hover:text-foreground",
                            "Inspect all targets →"
                        }
                    }

                    SectionRule { label: "State" }
                    div { class: "grid grid-cols-4 gap-px border border-border-soft bg-border-soft",
                        Tile { label: "In sync", value: count("in_sync"), tone: logic::Tone::Ok, note: "matching their assigned set", state: "in_sync" }
                        Tile { label: "Pending", value: count("pending"), tone: logic::Tone::Pending, note: "downloading or installing", state: "pending" }
                        Tile { label: "Error", value: count("error"), tone: logic::Tone::Error, note: "needs attention", state: "error" }
                        Tile { label: "Registered", value: count("registered"), tone: logic::Tone::Info, note: "known, nothing assigned", state: "registered" }
                    }

                    div { class: "mt-4 grid grid-cols-1 gap-4 xl:grid-cols-[1.6fr_1fr]",
                        div {
                            if !active_rollouts.is_empty() {
                                SectionRule { label: "Active rollouts" }
                                div { class: "border border-border-soft bg-card",
                                    for r in active_rollouts.clone() {
                                        div { key: "{r.id}", class: "border-b border-border-subtle p-4 last:border-b-0",
                                            div { class: "mb-2 flex flex-wrap items-baseline justify-between gap-2",
                                                Link {
                                                    to: Route::RolloutDetail { id: r.id },
                                                    class: "font-display text-base font-bold tracking-wide uppercase text-primary hover:underline",
                                                    "{r.name}"
                                                }
                                                span { class: "flex items-center gap-2 font-mono text-xs text-muted-foreground",
                                                    "{r.total_targets_per_status.finished} of {r.total_targets} finished"
                                                    StatusBadge { status: r.status.clone() }
                                                }
                                            }
                                            ProgressBar { counts: r.total_targets_per_status, total: r.total_targets }
                                            div { class: "mt-2",
                                                ProgressLegend { counts: r.total_targets_per_status }
                                            }
                                        }
                                    }
                                }
                            }

                            // The panel the old dashboard had no equivalent of: it
                            // listed recent *actions*, so a device stuck for an
                            // hour never appeared anywhere.
                            SectionRule { label: "Needs attention" }
                            div { class: "border border-border-soft bg-card",
                                if worst.is_empty() {
                                    p { class: "p-6 text-center text-sm text-muted-foreground",
                                        "Nothing failing. Every target is in sync or working through an assignment."
                                    }
                                } else {
                                    table { class: TABLE,
                                        thead {
                                            tr {
                                                th { class: TH, "Controller ID" }
                                                th { class: TH, "Name" }
                                                th { class: TH, "State" }
                                                th { class: "{TH} text-right", "Last poll" }
                                            }
                                        }
                                        tbody {
                                            for t in worst.clone() {
                                                tr { key: "{t.controller_id}", class: ROW,
                                                    td { class: TD,
                                                        Link {
                                                            to: Route::TargetDetail { cid: t.controller_id.clone() },
                                                            class: "{LINK_CELL} font-mono text-xs",
                                                            "{t.controller_id}"
                                                        }
                                                    }
                                                    td { class: TD, "{t.name}" }
                                                    td { class: TD, StatusBadge { status: t.update_status.clone() } }
                                                    td {
                                                        class: "{TD} text-right font-mono text-xs text-err",
                                                        title: {t.last_controller_request_at.map(logic::format_ts).unwrap_or_default()},
                                                        {logic::relative_age(now, t.last_controller_request_at)}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        div {
                            SegmentPanel { segments }

                            SectionRule { label: "Recent actions" }
                            div { class: "border border-border-soft bg-card",
                                if recent.content.is_empty() {
                                    p { class: "p-6 text-center text-sm text-muted-foreground", "No actions yet." }
                                } else {
                                    table { class: TABLE,
                                        tbody {
                                            for a in recent.content.clone() {
                                                tr { key: "{a.id}",
                                                    td { class: "{TD} font-mono text-xs text-muted-foreground", "#{a.id}" }
                                                    td { class: TD,
                                                        if let Some(cid) = a.target.clone() {
                                                            Link {
                                                                to: Route::TargetDetail { cid: cid.clone() },
                                                                class: "{LINK_CELL} font-mono text-xs",
                                                                "{cid}"
                                                            }
                                                        } else {
                                                            span { class: "text-muted-foreground", "—" }
                                                        }
                                                    }
                                                    td { class: "{TD} font-mono text-xs text-muted-foreground", "{a.action_type}" }
                                                    td { class: TD, StatusBadge { status: a.detail_status.clone() } }
                                                    td { class: "{TD} text-right font-mono text-xs text-muted-foreground",
                                                        {logic::format_ts(a.last_modified_at)}
                                                    }
                                                }
                                            }
                                        }
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
        SectionRule { label: "System" }
        SystemCard { configs }
    }
}

/// One row per saved target filter: how big the segment is, how much of it is
/// where it should be, and which set is auto-assigned to it.
///
/// There is deliberately no fleet-wide version histogram anywhere on this page.
/// raptor assigns distribution *sets*, and a Linux gateway's `os_app` set shares
/// no version axis with a sensor's `os` set — summing them produces a number that
/// means nothing. Per-segment "is it where it should be" is the comparable thing.
#[component]
fn SegmentPanel(segments: Resource<api::ApiResult<Segments>>) -> Element {
    rsx! {
        SectionRule { label: "Fleet segments" }
        div { class: "border border-border-soft bg-card",
            match &*segments.read_unchecked() {
                Some(Ok(s)) if s.rows.is_empty() => rsx! {
                    p { class: "p-6 text-center text-sm text-muted-foreground",
                        "No saved target filters. Save one to track a slice of the fleet — a device class, a site, a release channel — as its own segment."
                    }
                },
                Some(Ok(s)) => rsx! {
                    for (f , st) in s.rows.iter().cloned() {
                        SegmentRow {
                            key: "{f.id}",
                            ds_name: f.auto_assign_distribution_set.and_then(|id| s.ds_names.get(&id).cloned()),
                            filter: f,
                            stats: st,
                        }
                    }
                    div { class: "flex items-center justify-between border-t border-border-soft px-4 py-2 font-mono text-[11px] text-muted-foreground",
                        span { "Saved target filters" }
                        Link {
                            to: Route::target_filters(),
                            class: "text-fg-dim underline underline-offset-4 hover:text-foreground",
                            "Manage →"
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "p-4 text-sm text-err", "{e}" } },
                None => rsx! { p { class: "p-4 text-sm text-muted-foreground", "Loading segments…" } },
            }
        }
    }
}

#[component]
fn SegmentRow(
    filter: TargetFilterRest,
    stats: Option<SystemStatistics>,
    ds_name: Option<String>,
) -> Element {
    let total = stats.as_ref().map(|s| s.total_targets).unwrap_or(0);
    let of = |k: &str| {
        stats
            .as_ref()
            .and_then(|s| s.targets_by_status.get(k).copied())
            .unwrap_or(0)
    };
    let pct = |n: u64| {
        if total == 0 {
            0.0
        } else {
            n as f64 * 100.0 / total as f64
        }
    };
    let auto = logic::auto_assign_label(
        filter.auto_assign_distribution_set,
        ds_name.as_deref(),
        filter.auto_assign_action_type.as_deref(),
    );
    let (in_sync, pending, errors, registered) =
        (of("in_sync"), of("pending"), of("error"), of("registered"));
    let bar_label = format!(
        "{}: {in_sync} in sync, {pending} pending, {errors} error, {registered} registered",
        filter.name
    );
    rsx! {
        div { class: "border-b border-border-subtle p-4 last:border-b-0",
            div { class: "flex items-baseline justify-between gap-3",
                span { class: "font-medium text-foreground", "{filter.name}" }
                span { class: "font-mono text-xs text-muted-foreground", "{total} targets" }
            }
            p { class: "mt-0.5 font-mono text-[11px] break-all text-muted-foreground", "{filter.query}" }
            if stats.is_none() {
                p { class: "mt-2 font-mono text-[11px] text-pend",
                    "counts unavailable — this filter's query did not compile"
                }
            } else {
                // Fills, not outlines: the band is the encoding.
                div {
                    class: "mt-2 flex h-2 overflow-hidden bg-border-soft",
                    role: "img",
                    aria_label: "{bar_label}",
                    span { class: "bg-ok", style: "width: {pct(in_sync)}%" }
                    span { class: "bg-pend", style: "width: {pct(pending)}%" }
                    span { class: "bg-err", style: "width: {pct(errors)}%" }
                    span { class: "bg-info", style: "width: {pct(registered)}%" }
                }
                div { class: "mt-2 flex flex-wrap items-center gap-3 font-mono text-[11px]",
                    span { class: if errors > 0 { "text-err" } else { "text-muted-foreground" }, "{errors} failing" }
                    span { class: "text-muted-foreground", "{pending} in flight" }
                    span { class: "ml-auto",
                        match auto {
                            Some(a) => rsx! {
                                span { class: "text-fg-dim", "{a} · auto" }
                            },
                            None => rsx! {
                                span { class: "text-muted-foreground", "no auto-assign" }
                            },
                        }
                    }
                }
            }
        }
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

/// A fleet counter. The tick scale under the number is what makes the row read as
/// instrumentation rather than as four generic cards.
///
/// Takes a [`Tone`] rather than class strings so the number and its lamp cannot
/// drift apart, and so both class names stay literal — Tailwind scans source
/// text, so an interpolated `"text-{tone}"` would generate no rule and the figure
/// would render unstyled.
#[component]
fn Tile(
    label: String,
    value: u64,
    tone: logic::Tone,
    note: String,
    state: &'static str,
) -> Element {
    rsx! {
        Link {
            to: Route::Targets { query: String::new(), state: state.to_string(), tag: String::new(), offset: 0 },
            class: "tick-scale relative block bg-card p-4 hover:bg-accent",
            p { class: "font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                "{label}"
            }
            p {
                class: "font-display mt-1.5 text-[40px] leading-none font-bold tabular-nums {tone_text(tone)}",
                "{value}"
            }
            p { class: "mt-1.5 flex items-center gap-1.5 font-mono text-[11px] text-muted-foreground",
                span { class: "inline-block size-[7px] rounded-full {tone_fill(tone)}" }
                "{note}"
            }
        }
    }
}
