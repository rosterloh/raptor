//! Quota enforcement: the one place a configured `[quota]` limit turns into a
//! rejection, so every call site reports violations identically.
//!
//! Mirrors hawkBit's `QuotaHelper.assertAssignmentQuota` — including its
//! treatment of any limit `<= 0` as "unlimited", which is how an operator turns
//! a quota off — and reports breaches as `429 Too Many Requests` with
//! `hawkbit.server.error.quota.tooManyEntries`.

use crate::error::AppError;

/// Rejects an operation that would push `current + requested` past `limit`.
///
/// `limit == 0` disables the quota. `what` names the entity being added and
/// `parent` what it is being added to, both only for the error message —
/// phrased like hawkBit's so an operator who has seen its errors recognises
/// these.
///
/// Checked *before* inserting rather than after, so a request that would breach
/// the quota is rejected whole instead of leaving a partial write behind.
pub fn assert_quota(
    current: u64,
    requested: u64,
    limit: u32,
    what: &str,
    parent: &str,
) -> Result<(), AppError> {
    if limit == 0 {
        return Ok(());
    }
    let limit = u64::from(limit);
    if current + requested > limit {
        return Err(AppError::QuotaExceeded(format!(
            "Quota exceeded: cannot add {requested} more {what} entries to {parent}. \
             The maximum is {limit}, and there are already {current}."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(current: u64, requested: u64, limit: u32) -> Result<(), AppError> {
        assert_quota(current, requested, limit, "artifact", "software module 1")
    }

    #[test]
    fn allows_up_to_and_including_the_limit() {
        assert!(check(0, 1, 50).is_ok());
        assert!(
            check(49, 1, 50).is_ok(),
            "landing exactly on the limit is ok"
        );
        assert!(check(50, 0, 50).is_ok(), "a no-op request never breaches");
    }

    #[test]
    fn rejects_the_entry_that_would_cross_the_limit() {
        assert!(matches!(check(50, 1, 50), Err(AppError::QuotaExceeded(_))));
        // A batch is rejected whole, not partially applied up to the limit.
        assert!(matches!(check(0, 51, 50), Err(AppError::QuotaExceeded(_))));
        assert!(matches!(check(48, 3, 50), Err(AppError::QuotaExceeded(_))));
    }

    /// hawkBit treats any limit <= 0 as unlimited; raptor's config is unsigned,
    /// so 0 is the whole of that case and must not reject anything.
    #[test]
    fn zero_disables_the_quota() {
        assert!(check(0, 1, 0).is_ok());
        assert!(check(u64::MAX - 1, 1, 0).is_ok());
    }

    /// The message names the numbers an operator needs to act — what they asked
    /// for, the cap, and how full it already is.
    #[test]
    fn the_message_carries_the_numbers() {
        let Err(AppError::QuotaExceeded(m)) = check(50, 1, 50) else {
            panic!("expected a quota error");
        };
        assert!(m.contains("artifact"), "{m}");
        assert!(m.contains("software module 1"), "{m}");
        assert!(m.contains("maximum is 50"), "{m}");
        assert!(m.contains("already 50"), "{m}");
    }
}
