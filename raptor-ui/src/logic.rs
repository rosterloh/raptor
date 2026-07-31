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

/// ANDs FIQL terms, parenthesising each so an OR group from
/// [`fiql_contains`] keeps its meaning — `;` binds tighter than `,`, so
/// `a==1,b==1;tag==x` would otherwise read as `a==1 OR (b==1 AND tag==x)`.
pub fn fiql_and(terms: &[Option<String>]) -> Option<String> {
    let parts: Vec<String> = terms.iter().flatten().map(|t| format!("({t})")).collect();
    match parts.len() {
        0 => None,
        _ => Some(parts.join(";")),
    }
}

/// The FIQL term selecting one tag by name. Tag names are user input, so the
/// value is quoted — a name containing `;`, `,` or a space would otherwise end
/// the term early.
pub fn fiql_tag(name: &str) -> Option<String> {
    let n = name.trim();
    (!n.is_empty()).then(|| format!("tag=='{}'", n.replace('\'', "")))
}

/// A tag's colour, validated for use in a `style` attribute. Colours are
/// free-form user input that would otherwise be interpolated into CSS, so
/// anything that isn't a plain `#rgb` / `#rrggbb` hex literal is dropped and
/// the chip falls back to its neutral styling.
pub fn tag_colour(colour: Option<&str>) -> Option<String> {
    let c = colour?.trim();
    let hex = c.strip_prefix('#')?;
    let valid = matches!(hex.len(), 3 | 6) && hex.bytes().all(|b| b.is_ascii_hexdigit());
    valid.then(|| format!("#{hex}"))
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

/// How long ago something happened, at a glance: `4s`, `16m`, `3h`, `12d`.
///
/// The question an operator asks of a poll timestamp is "is this device stale",
/// and an absolute clock time makes them do the arithmetic. Only ever used on
/// screens that poll, so the answer does not sit there going quietly wrong.
///
/// `None` renders as `never` — a target that has registered but never polled is
/// a different and worse condition than one that is merely late.
pub fn relative_age(now_ms: i64, then_ms: Option<i64>) -> String {
    let Some(then) = then_ms else {
        return "never".into();
    };
    // A clock skewed behind the server, or a timestamp from the future, must not
    // render as a huge negative age.
    let secs = ((now_ms - then) / 1000).max(0);
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

/// What a state *means*, independent of how it is painted.
///
/// This module is compiled and tested on the host and holds no presentation:
/// returning Tailwind class strings from here is what previously made a palette
/// change a code change, and what made a light theme impossible. Components map
/// a tone onto tokens; see `components::tone_badge` / `components::tone_fill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Where it should be: in sync, finished.
    Ok,
    /// Working on it: pending, running, downloading.
    Pending,
    /// Needs a human: error, stopped.
    Error,
    /// Known but not yet acted on: registered, scheduled.
    Info,
    /// No signal either way: ready, canceled, unknown.
    Neutral,
}

/// Segments of a rollout progress bar, most-advanced first: (label, tone,
/// count). Empty buckets are dropped so the bar has no zero-width slivers and
/// the legend only names states that actually occurred.
pub fn progress_segments(
    c: &raptor_api_types::RolloutTargetsPerStatus,
) -> Vec<(&'static str, Tone, i64)> {
    [
        ("finished", Tone::Ok, c.finished),
        ("running", Tone::Pending, c.running),
        ("error", Tone::Error, c.error),
        ("cancelled", Tone::Neutral, c.cancelled),
        ("scheduled", Tone::Info, c.scheduled),
        ("not started", Tone::Neutral, c.notstarted),
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

/// (display label, tone) for a target `updateStatus` or a rollout/group state.
///
/// The label is deliberately not just the raw key — `in_sync` reads as
/// "in sync" — because the badge shows a word next to its colour rather than
/// relying on colour alone.
pub fn status_style(update_status: &str) -> (&'static str, Tone) {
    match update_status {
        "in_sync" => ("in sync", Tone::Ok),
        "pending" => ("pending", Tone::Pending),
        "error" => ("error", Tone::Error),
        "registered" => ("registered", Tone::Info),
        // rollout / rollout-group lifecycle states
        "ready" => ("ready", Tone::Neutral),
        "scheduled" => ("scheduled", Tone::Info),
        "running" => ("running", Tone::Pending),
        "paused" => ("paused", Tone::Pending),
        "finished" => ("finished", Tone::Ok),
        "canceled" => ("canceled", Tone::Neutral),
        "stopped" => ("stopped", Tone::Error),
        _ => ("unknown", Tone::Neutral),
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

/// Tenant-configuration keys in the order the console shows them: the polling
/// interval first, then the workflow toggles, then the auth modes.
pub const CONFIG_ORDER: &[&str] = &[
    "pollingTime",
    "user.confirmation.flow.enabled",
    "rollout.approval.enabled",
    "multi.assignments.enabled",
    "authentication.targettoken.enabled",
    "authentication.gatewaytoken.enabled",
];

/// The keys to render, known ones in [`CONFIG_ORDER`] first. Keys the server
/// grows later are appended rather than dropped — they arrive from a
/// `BTreeMap`, so the tail stays alphabetical.
pub fn ordered_config_keys(keys: &[&str]) -> Vec<String> {
    let known = CONFIG_ORDER
        .iter()
        .filter(|k| keys.contains(k))
        .map(|k| (*k).to_string());
    let rest = keys
        .iter()
        .filter(|k| !CONFIG_ORDER.contains(k))
        .map(|k| (*k).to_string());
    known.chain(rest).collect()
}

/// Operator-facing label for a tenant-config key; unknown keys show verbatim.
pub fn config_label(key: &str) -> &str {
    match key {
        "pollingTime" => "Polling interval",
        "user.confirmation.flow.enabled" => "Confirmation flow",
        "rollout.approval.enabled" => "Rollout approval",
        "multi.assignments.enabled" => "Multi-assignment",
        "authentication.targettoken.enabled" => "Target token auth",
        "authentication.gatewaytoken.enabled" => "Gateway token auth",
        other => other,
    }
}

/// A tenant-config value as text. The flags are booleans and the polling
/// interval a string; anything else falls back to its JSON rendering.
pub fn format_config_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Bool(true) => "enabled".into(),
        serde_json::Value::Bool(false) => "disabled".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
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
    fn fiql_and_parenthesises_or_groups() {
        assert_eq!(
            fiql_and(&[fiql_contains(&["name"], "dev"), fiql_tag("beta")]),
            Some("(name==*dev*);(tag=='beta')".to_string())
        );
        // a single term still round-trips, and empties drop out
        assert_eq!(fiql_and(&[fiql_tag("beta")]), Some("(tag=='beta')".into()));
        assert_eq!(fiql_and(&[None, None]), None);
        assert_eq!(fiql_and(&[]), None);
    }

    #[test]
    fn fiql_tag_quotes_the_name() {
        assert_eq!(fiql_tag("eu west"), Some("tag=='eu west'".into()));
        assert_eq!(fiql_tag("  "), None);
        // a quote in the name can't break out of the quoted value
        assert_eq!(fiql_tag("a'b"), Some("tag=='ab'".into()));
    }

    #[test]
    fn tag_colour_accepts_only_hex() {
        assert_eq!(tag_colour(Some("#ff0000")), Some("#ff0000".into()));
        assert_eq!(tag_colour(Some(" #abc ")), Some("#abc".into()));
        assert_eq!(tag_colour(None), None);
        assert_eq!(tag_colour(Some("red")), None);
        assert_eq!(tag_colour(Some("#12345")), None);
        assert_eq!(tag_colour(Some("#zzzzzz")), None);
        // no CSS injection through the style attribute
        assert_eq!(tag_colour(Some("#fff;background:url(x)")), None);
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
    fn relative_age_scales_and_never_goes_negative() {
        let now = 1_000_000_000;
        let ago = |secs: i64| relative_age(now, Some(now - secs * 1000));
        assert_eq!(ago(0), "0s");
        assert_eq!(ago(45), "45s");
        assert_eq!(ago(60), "1m");
        assert_eq!(ago(59 * 60), "59m");
        assert_eq!(ago(3600), "1h");
        assert_eq!(ago(23 * 3600), "23h");
        assert_eq!(ago(86_400), "1d");
        assert_eq!(ago(400 * 86_400), "400d");
        // Never polled is its own answer, not "a very long time ago".
        assert_eq!(relative_age(now, None), "never");
        // A device clock ahead of ours, or ours behind the server, must not
        // produce a negative age.
        assert_eq!(relative_age(now, Some(now + 60_000)), "0s");
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
        assert_eq!(progress_segments(&c)[0].1, Tone::Ok);
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
            let (label, _tone) = status_style(s);
            assert!(!label.is_empty(), "{s} has no label");
        }
        // The states an operator must act on must not collapse into the same
        // tone as the states they can ignore.
        assert_eq!(status_style("error").1, Tone::Error);
        assert_eq!(status_style("stopped").1, Tone::Error);
        assert_eq!(status_style("in_sync").1, Tone::Ok);
        assert_eq!(status_style("???").1, Tone::Neutral);
        // No Tailwind class ever leaves this module again — that is what makes a
        // palette change an edit to tailwind.css rather than to Rust.
        assert!(!format!("{:?}", status_style("error")).contains("bg-"));
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

    #[test]
    fn config_keys_order_known_first_and_keep_the_rest() {
        // as the server sends them: a BTreeMap, so alphabetical
        let keys = [
            "authentication.gatewaytoken.enabled",
            "pollingTime",
            "some.future.key",
            "user.confirmation.flow.enabled",
        ];
        assert_eq!(
            ordered_config_keys(&keys),
            vec![
                "pollingTime",
                "user.confirmation.flow.enabled",
                "authentication.gatewaytoken.enabled",
                "some.future.key",
            ]
        );
        assert!(ordered_config_keys(&[]).is_empty());
    }

    #[test]
    fn config_values_render_as_text() {
        use serde_json::json;
        assert_eq!(format_config_value(&json!(true)), "enabled");
        assert_eq!(format_config_value(&json!(false)), "disabled");
        assert_eq!(format_config_value(&json!("30s")), "30s");
        assert_eq!(format_config_value(&json!(5)), "5");
        // unknown keys have no friendly label and show as sent
        assert_eq!(config_label("pollingTime"), "Polling interval");
        assert_eq!(config_label("some.future.key"), "some.future.key");
    }
}
