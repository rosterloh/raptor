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

/// Which field of the target-filter form an API write error belongs to. The
/// server validates FIQL at write time (400) and rejects duplicate names (409),
/// so the status alone says where the message goes; anything else is a
/// form-level failure worth a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterField {
    Name,
    Query,
}

pub fn filter_error_field(status: u16) -> Option<FilterField> {
    match status {
        400 => Some(FilterField::Query),
        409 => Some(FilterField::Name),
        _ => None,
    }
}

/// Label for a filter's auto-assign column: the distribution set plus its action
/// type, or `None` when nothing is attached. `ds` is the resolved set name; it is
/// absent when the set is outside the page of sets the list page loaded, in which
/// case the id stands in.
pub fn auto_assign_label(
    ds_id: Option<i64>,
    ds: Option<&str>,
    action_type: Option<&str>,
) -> Option<String> {
    let id = ds_id?;
    let name = ds
        .map(str::to_string)
        .unwrap_or_else(|| format!("DS #{id}"));
    Some(match action_type.filter(|t| !t.is_empty()) {
        Some(t) => format!("{name} · {t}"),
        None => name,
    })
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

    #[test]
    fn filter_errors_map_to_their_field() {
        assert_eq!(filter_error_field(400), Some(FilterField::Query));
        assert_eq!(filter_error_field(409), Some(FilterField::Name));
        assert_eq!(filter_error_field(404), None);
        assert_eq!(filter_error_field(500), None);
    }

    #[test]
    fn auto_assign_labels_cover_missing_pieces() {
        assert_eq!(
            auto_assign_label(Some(7), Some("fleet 1.0"), Some("forced")),
            Some("fleet 1.0 · forced".into())
        );
        assert_eq!(
            auto_assign_label(Some(7), None, Some("soft")),
            Some("DS #7 · soft".into())
        );
        assert_eq!(
            auto_assign_label(Some(7), Some("fleet 1.0"), None),
            Some("fleet 1.0".into())
        );
        assert_eq!(auto_assign_label(None, None, Some("forced")), None);
    }
}
