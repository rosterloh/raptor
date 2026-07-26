//! Pure logic, unit-tested on the host: `cargo test -p raptor-ui`.

/// FIQL "contains" filter ORed over fields: `name==*term*,controllerId==*term*`.
/// raptor's FIQL compiler maps `*` wildcards to SQL LIKE.
pub fn fiql_contains(fields: &[&str], term: &str) -> Option<String> {
    let t = term.trim();
    if t.is_empty() {
        return None;
    }
    Some(
        fields
            .iter()
            .map(|f| format!("{f}==*{t}*"))
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// Percent-encode everything outside RFC 3986 unreserved characters.
pub fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn format_ts(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".into())
}

/// Segments of a rollout progress bar, most-advanced first: (label, bar colour
/// class, count). Empty buckets are dropped so the bar has no zero-width slivers
/// and the legend only names states that actually occurred.
pub fn progress_segments(
    c: &raptor_api_types::RolloutTargetsPerStatus,
) -> Vec<(&'static str, &'static str, i64)> {
    [
        ("finished", "bg-emerald-500", c.finished),
        ("running", "bg-sky-500", c.running),
        ("error", "bg-red-500", c.error),
        ("cancelled", "bg-zinc-500", c.cancelled),
        ("scheduled", "bg-zinc-600", c.scheduled),
        ("not started", "bg-zinc-700", c.notstarted),
    ]
    .into_iter()
    .filter(|(_, _, n)| *n > 0)
    .collect()
}

/// `n` as a percentage of `total`, clamped to 0–100 (0 when `total` is 0) —
/// used as a CSS width, where a NaN or out-of-range value would break layout.
pub fn percent(n: i64, total: i64) -> f64 {
    if total <= 0 {
        return 0.0;
    }
    (n as f64 * 100.0 / total as f64).clamp(0.0, 100.0)
}

/// (label, badge classes) for a target updateStatus.
pub fn status_style(update_status: &str) -> (&'static str, &'static str) {
    match update_status {
        "in_sync" => (
            "in sync",
            "bg-emerald-950 text-emerald-300 border-emerald-800",
        ),
        "pending" => ("pending", "bg-amber-950 text-amber-300 border-amber-800"),
        "error" => ("error", "bg-red-950 text-red-300 border-red-800"),
        "registered" => ("registered", "bg-sky-950 text-sky-300 border-sky-800"),
        // rollout / rollout-group lifecycle states
        "ready" => ("ready", "bg-zinc-800 text-zinc-300 border-zinc-700"),
        "scheduled" => ("scheduled", "bg-sky-950 text-sky-300 border-sky-800"),
        "running" => ("running", "bg-sky-950 text-sky-300 border-sky-800"),
        "paused" => ("paused", "bg-amber-950 text-amber-300 border-amber-800"),
        "finished" => (
            "finished",
            "bg-emerald-950 text-emerald-300 border-emerald-800",
        ),
        "canceled" => ("canceled", "bg-zinc-800 text-zinc-300 border-zinc-700"),
        "stopped" => ("stopped", "bg-red-950 text-red-300 border-red-800"),
        _ => ("unknown", "bg-zinc-800 text-zinc-300 border-zinc-700"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fiql_builds_wildcard_or_query() {
        assert_eq!(
            fiql_contains(&["name", "controllerId"], "dev"),
            Some("name==*dev*,controllerId==*dev*".to_string())
        );
        assert_eq!(fiql_contains(&["name"], "  "), None);
        assert_eq!(fiql_contains(&["name"], ""), None);
    }

    #[test]
    fn urlencode_escapes_reserved() {
        assert_eq!(urlencode("name==*a b*"), "name%3D%3D%2Aa%20b%2A");
        assert_eq!(urlencode("plain-safe_1.0~x"), "plain-safe_1.0~x");
    }

    #[test]
    fn timestamps_render() {
        assert_eq!(format_ts(0), "1970-01-01 00:00");
    }

    #[test]
    fn progress_segments_drop_empty_buckets_and_keep_order() {
        let c = raptor_api_types::RolloutTargetsPerStatus {
            notstarted: 0,
            scheduled: 4,
            running: 2,
            error: 1,
            finished: 3,
            cancelled: 0,
        };
        let labels: Vec<_> = progress_segments(&c).iter().map(|s| s.0).collect();
        assert_eq!(labels, ["finished", "running", "error", "scheduled"]);
        assert_eq!(progress_segments(&c)[0].2, 3);
        assert!(progress_segments(&Default::default()).is_empty());
    }

    #[test]
    fn percent_is_bounded() {
        assert_eq!(percent(3, 4), 75.0);
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(percent(5, -1), 0.0);
        assert_eq!(percent(9, 4), 100.0);
    }

    #[test]
    fn status_style_covers_known_states() {
        for s in [
            "in_sync",
            "pending",
            "error",
            "registered",
            "ready",
            "scheduled",
            "running",
            "paused",
            "finished",
            "canceled",
            "unknown",
            "???",
        ] {
            let (label, classes) = status_style(s);
            assert!(!label.is_empty());
            assert!(classes.contains("bg-"));
        }
    }
}
