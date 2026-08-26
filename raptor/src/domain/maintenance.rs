//! Maintenance windows on assignments: the device downloads as soon as the
//! action is assigned, but only installs while a recurring window is open.
//!
//! An action carries the window as the three fields hawkBit spells on
//! `maintenanceWindow` — a Quartz cron `schedule`, an `HH:mm:ss` `duration`,
//! and a `±HH:mm` `timezone` offset — either all set or all absent. Whether the
//! window is open right now is derived at request time rather than stored, so
//! nothing has to sweep actions as windows open and close: see
//! [`Window::is_open_at`] and its caller in `api::ddi::deployment`.

use crate::entity::action;
use crate::error::AppError;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use croner::Cron;
use croner::parser::{CronParser, Seconds, Year};

/// hawkBit expresses the schedule in Quartz cron, which differs from Unix cron
/// in three ways that all matter here: it leads with a seconds field (six or
/// seven fields, the seventh being an optional year), it allows `?` as the
/// "no specific value" wildcard in the two day fields, and it numbers weekdays
/// 1 = Sunday through 7 = Saturday rather than 0 = Sunday. `alternative_weekdays`
/// is croner's name for that last one — without it, every day-of-week schedule
/// would silently land one day early.
fn quartz() -> CronParser {
    CronParser::builder()
        .seconds(Seconds::Required)
        .year(Year::Optional)
        .alternative_weekdays(true)
        .build()
}

/// A validated maintenance window.
#[derive(Debug)]
pub struct Window {
    cron: Cron,
    duration_ms: i64,
    offset: FixedOffset,
}

/// `HH:mm:ss` as hawkBit writes durations, capped at `23:59:59` like theirs.
fn parse_duration(s: &str) -> Result<i64, AppError> {
    let bad = || AppError::BadRequest(format!("invalid maintenance window duration: {s}"));
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.len() != 2) {
        return Err(bad());
    }
    let n: Vec<i64> = parts
        .iter()
        .map(|p| p.parse::<i64>().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let (h, m, sec) = (n[0], n[1], n[2]);
    if h > 23 || m > 59 || sec > 59 {
        return Err(bad());
    }
    let ms = ((h * 3600) + (m * 60) + sec) * 1000;
    if ms == 0 {
        return Err(AppError::BadRequest(
            "maintenance window duration must be greater than zero".into(),
        ));
    }
    Ok(ms)
}

/// A `±HH:mm` offset from UTC. Deliberately stricter than chrono's parser,
/// which also accepts forms hawkBit never emits (`+0200`, `Z`) — a device that
/// installs an hour late because an offset was misread is a bad failure.
fn parse_offset(s: &str) -> Result<FixedOffset, AppError> {
    let bad = || AppError::BadRequest(format!("invalid maintenance window timezone: {s}"));
    let b = s.as_bytes();
    if b.len() != 6 || (b[0] != b'+' && b[0] != b'-') || b[3] != b':' {
        return Err(bad());
    }
    let h: i32 = s[1..3].parse().map_err(|_| bad())?;
    let m: i32 = s[4..6].parse().map_err(|_| bad())?;
    if h > 18 || m > 59 {
        return Err(bad());
    }
    let secs = (h * 3600 + m * 60) * if b[0] == b'-' { -1 } else { 1 };
    FixedOffset::east_opt(secs).ok_or_else(bad)
}

impl Window {
    /// Parses the three fields. Strictly a syntax check: a window that parses
    /// can always be evaluated, even if it will never open again. Whether it is
    /// *worth* setting is a separate question — see [`validate_assignable`].
    pub fn parse(schedule: &str, duration: &str, timezone: &str) -> Result<Self, AppError> {
        let cron = quartz().parse(schedule).map_err(|e| {
            AppError::BadRequest(format!("invalid maintenance window schedule: {e}"))
        })?;
        Ok(Window {
            cron,
            duration_ms: parse_duration(duration)?,
            offset: parse_offset(timezone)?,
        })
    }

    fn at(&self, ms: i64) -> Option<DateTime<FixedOffset>> {
        Utc.timestamp_millis_opt(ms)
            .single()
            .map(|t| t.with_timezone(&self.offset))
    }

    /// Whether the window is open at `now_ms`: the most recent start at or
    /// before `now_ms` is less than `duration` ago.
    pub fn is_open_at(&self, now_ms: i64) -> bool {
        let Some(now) = self.at(now_ms) else {
            return false;
        };
        // No previous occurrence at all — the first window is still ahead.
        let Ok(start) = self.cron.find_previous_occurrence(&now, true) else {
            return false;
        };
        now_ms - start.timestamp_millis() < self.duration_ms
    }

    /// Start of the next window at or after `now_ms`, as epoch millis —
    /// hawkBit's `nextStartAt`.
    pub fn next_start_at(&self, now_ms: i64) -> Option<i64> {
        let now = self.at(now_ms)?;
        self.cron
            .find_next_occurrence(&now, true)
            .ok()
            .map(|t| t.timestamp_millis())
    }
}

/// Validates a window an operator is trying to set. Beyond parsing, a schedule
/// pinned to a year already past can never open again, so hawkBit rejects it at
/// assignment time rather than leaving a device waiting forever.
///
/// This is deliberately *not* part of [`Window::parse`]: a stored window whose
/// last occurrence has since passed must keep evaluating as shut, never decay
/// into "no window at all" and let the install through unguarded.
pub fn validate_assignable(w: &raptor_api_types::MaintenanceWindowRequest) -> Result<(), AppError> {
    let parsed = Window::parse(&w.schedule, &w.duration, &w.timezone)?;
    if parsed.next_start_at(crate::util::now_ms()).is_none() {
        return Err(AppError::BadRequest(
            "maintenance window schedule has no future occurrence".into(),
        ));
    }
    Ok(())
}

/// The window an action carries, if it has one. The three columns are written
/// together, so a partial row is treated as no window rather than guessed at.
pub fn window_for(a: &action::Model) -> Option<Window> {
    let (Some(s), Some(d), Some(tz)) = (
        a.maintenance_schedule.as_deref(),
        a.maintenance_duration.as_deref(),
        a.maintenance_timezone.as_deref(),
    ) else {
        return None;
    };
    Window::parse(s, d, tz).ok()
}

/// The DDI `deployment.maintenanceWindow` value for an action: `None` when the
/// action has no window, in which case the field is omitted entirely and the
/// payload is byte-identical to one from before this feature existed.
pub fn availability(a: &action::Model, now_ms: i64) -> Option<&'static str> {
    let w = window_for(a)?;
    Some(if w.is_open_at(now_ms) {
        "available"
    } else {
        "unavailable"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Epoch millis for a UTC wall-clock instant.
    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        chrono::NaiveDate::from_ymd_opt(y, mo, d)
            .unwrap()
            .and_hms_opt(h, mi, s)
            .unwrap()
            .and_utc()
            .timestamp_millis()
    }

    fn win(schedule: &str, duration: &str, tz: &str) -> Window {
        Window::parse(schedule, duration, tz).expect("should parse")
    }

    #[test]
    fn window_is_open_from_its_start_until_the_duration_elapses() {
        // 02:00 UTC every day, for two hours.
        let w = win("0 0 2 * * ?", "02:00:00", "+00:00");
        assert!(!w.is_open_at(utc(2026, 8, 23, 1, 59, 59)));
        assert!(w.is_open_at(utc(2026, 8, 23, 2, 0, 0)));
        assert!(w.is_open_at(utc(2026, 8, 23, 3, 59, 59)));
        // Exactly one duration after the start is already closed.
        assert!(!w.is_open_at(utc(2026, 8, 23, 4, 0, 0)));
    }

    #[test]
    fn window_is_evaluated_in_its_own_timezone() {
        // 02:00 in +02:00 is 00:00 UTC.
        let w = win("0 0 2 * * ?", "01:00:00", "+02:00");
        assert!(w.is_open_at(utc(2026, 8, 23, 0, 30, 0)));
        assert!(!w.is_open_at(utc(2026, 8, 23, 2, 30, 0)));

        // ...and 02:00 in -05:00 is 07:00 UTC.
        let w = win("0 0 2 * * ?", "01:00:00", "-05:00");
        assert!(!w.is_open_at(utc(2026, 8, 23, 0, 30, 0)));
        assert!(w.is_open_at(utc(2026, 8, 23, 7, 30, 0)));
    }

    /// The Quartz-vs-Unix trap: `2` is Monday in Quartz, Tuesday in Unix cron.
    /// 2026-08-24 is a Monday.
    #[test]
    fn day_of_week_uses_quartz_numbering() {
        let w = win("0 0 2 ? * 2", "01:00:00", "+00:00");
        assert!(w.is_open_at(utc(2026, 8, 24, 2, 30, 0)), "Monday");
        assert!(!w.is_open_at(utc(2026, 8, 25, 2, 30, 0)), "Tuesday");

        // Names resolve to the same day, which is the cross-check that the
        // numbering is Quartz's rather than coincidence.
        let named = win("0 0 2 ? * MON", "01:00:00", "+00:00");
        assert!(named.is_open_at(utc(2026, 8, 24, 2, 30, 0)));
    }

    #[test]
    fn next_start_at_reports_the_upcoming_window() {
        let w = win("0 0 2 * * ?", "01:00:00", "+00:00");
        assert_eq!(
            w.next_start_at(utc(2026, 8, 23, 5, 0, 0)),
            Some(utc(2026, 8, 24, 2, 0, 0))
        );
        // Inside a window, the current start is the next one.
        assert_eq!(
            w.next_start_at(utc(2026, 8, 23, 2, 30, 0)),
            Some(utc(2026, 8, 24, 2, 0, 0))
        );
    }

    fn request(schedule: &str) -> raptor_api_types::MaintenanceWindowRequest {
        raptor_api_types::MaintenanceWindowRequest {
            schedule: schedule.into(),
            duration: "01:00:00".into(),
            timezone: "+00:00".into(),
        }
    }

    #[test]
    fn seven_field_schedules_carry_a_year() {
        assert!(Window::parse("0 15 10 * * ? 2199", "01:00:00", "+00:00").is_ok());
        assert!(validate_assignable(&request("0 15 10 * * ? 2199")).is_ok());
        // A year already past can never open again, so it is not assignable.
        let e = validate_assignable(&request("0 15 10 * * ? 2018")).unwrap_err();
        assert!(matches!(e, AppError::BadRequest(_)));
    }

    /// A window whose last occurrence has passed must stay shut, not decay into
    /// "no window" and let the install through unguarded. Only assignment-time
    /// validation rejects an exhausted schedule; evaluation still honours it.
    #[test]
    fn an_exhausted_window_evaluates_as_shut_rather_than_absent() {
        let w = win("0 15 10 * * ? 2018", "01:00:00", "+00:00");
        assert!(!w.is_open_at(utc(2026, 8, 23, 10, 30, 0)));
        assert_eq!(w.next_start_at(utc(2026, 8, 23, 10, 30, 0)), None);

        let a = action_with(Some("0 15 10 * * ? 2018"));
        assert_eq!(
            availability(&a, utc(2026, 8, 23, 10, 30, 0)),
            Some("unavailable")
        );
    }

    /// An action row with a window, for the entity-level helpers.
    fn action_with(schedule: Option<&str>) -> action::Model {
        action::Model {
            id: 1,
            target_id: 1,
            ds_id: 1,
            status: "running".into(),
            active: true,
            action_type: "forced".into(),
            forced_time: None,
            rollout_id: None,
            rollout_group_id: None,
            created_at: 0,
            updated_at: 0,
            deployment_fetch_count: 0,
            maintenance_schedule: schedule.map(str::to_string),
            maintenance_duration: schedule.map(|_| "01:00:00".to_string()),
            maintenance_timezone: schedule.map(|_| "+00:00".to_string()),
            tenant: "DEFAULT".into(),
        }
    }

    #[test]
    fn an_action_without_a_window_reports_no_availability_at_all() {
        assert_eq!(
            availability(&action_with(None), utc(2026, 8, 23, 2, 30, 0)),
            None
        );
    }

    #[test]
    fn malformed_fields_are_rejected() {
        for (s, d, tz) in [
            ("not a cron", "01:00:00", "+00:00"),
            // Five fields: Unix cron, missing Quartz's leading seconds.
            ("0 2 * * ?", "01:00:00", "+00:00"),
            ("0 0 2 * * ?", "1:00:00", "+00:00"),
            ("0 0 2 * * ?", "24:00:00", "+00:00"),
            ("0 0 2 * * ?", "00:60:00", "+00:00"),
            ("0 0 2 * * ?", "00:00:00", "+00:00"),
            ("0 0 2 * * ?", "01:00:00", "+0200"),
            ("0 0 2 * * ?", "01:00:00", "Z"),
            ("0 0 2 * * ?", "01:00:00", "02:00"),
            ("0 0 2 * * ?", "01:00:00", "+99:00"),
        ] {
            assert!(
                Window::parse(s, d, tz).is_err(),
                "expected {s:?}/{d:?}/{tz:?} to be rejected"
            );
        }
    }
}
