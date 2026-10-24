//! `Range: bytes=...` resolution for artifact downloads (RFC 9110 §14).
//!
//! Pure parsing and coalescing; [`super::artifacts`] turns the result into a
//! `200`, a single-part `206`, a `multipart/byteranges` `206`, or a `416`.

use std::ops::Range;

/// Upper bound on parts in one multipart response, after coalescing. SWUpdate's
/// delta handler sends at most `max-ranges` (default 10); Apache allows 200.
pub const MAX_PARTS: usize = 256;

/// What to serve for a `Range` header against a blob of a given size.
#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Ignore the header and send the whole blob with `200`.
    Full,
    /// No range-spec overlaps the blob: `416`.
    Unsatisfiable,
    /// Half-open ranges in request order; never empty.
    Parts(Vec<Range<u64>>),
}

/// Resolve a `Range` header value against a blob of `size` bytes.
///
/// Parts keep the order they were requested in, because SWUpdate's delta
/// handler matches parts to its own request list and never reads the per-part
/// `Content-Range`. A range that overlaps or abuts the one before it, and does
/// not start before it, is merged into it.
///
/// The header is ignored, as RFC 9110 §14.2 allows, when it is malformed,
/// names a unit other than `bytes`, needs more than [`MAX_PARTS`] parts, or
/// asks for more bytes in total than the blob holds (overlapping ranges, which
/// would otherwise multiply the bytes read for one request).
pub fn resolve(header: &str, size: u64) -> Resolved {
    let Some(specs) = parse(header) else {
        return Resolved::Full;
    };
    let mut parts: Vec<Range<u64>> = Vec::with_capacity(specs.len());
    for r in specs.into_iter().filter_map(|s| s.clamp(size)) {
        match parts.last_mut() {
            Some(last) if last.start <= r.start && r.start <= last.end => {
                last.end = last.end.max(r.end)
            }
            _ => parts.push(r),
        }
    }
    if parts.is_empty() {
        return Resolved::Unsatisfiable;
    }
    let requested: u64 = parts.iter().map(|r| r.end - r.start).sum();
    if parts.len() > MAX_PARTS || requested > size {
        return Resolved::Full;
    }
    Resolved::Parts(parts)
}

#[derive(Debug, PartialEq, Eq)]
enum Spec {
    /// `first-` or `first-last` (inclusive).
    From { first: u64, last: Option<u64> },
    /// `-n`: the final `n` bytes.
    Suffix(u64),
}

impl Spec {
    fn clamp(self, size: u64) -> Option<Range<u64>> {
        match self {
            Spec::From { first, last } => {
                let end = last.map_or(size, |l| l.saturating_add(1).min(size));
                (first < size).then_some(first..end)
            }
            Spec::Suffix(n) => (n > 0 && size > 0).then(|| size - n.min(size)..size),
        }
    }
}

fn parse(header: &str) -> Option<Vec<Spec>> {
    let (unit, set) = header.trim().split_once('=')?;
    if !unit.trim().eq_ignore_ascii_case("bytes") {
        return None;
    }
    let mut specs = Vec::new();
    // RFC 9110 list syntax tolerates empty elements ("a-b, ,c-d").
    for item in set.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (first, last) = item.split_once('-')?;
        let spec = match (first.trim(), last.trim()) {
            ("", n) => Spec::Suffix(digits(n)?),
            (f, "") => Spec::From {
                first: digits(f)?,
                last: None,
            },
            (f, l) => {
                let (first, last) = (digits(f)?, digits(l)?);
                if last < first {
                    return None;
                }
                Spec::From {
                    first,
                    last: Some(last),
                }
            }
        };
        specs.push(spec);
    }
    (!specs.is_empty()).then_some(specs)
}

/// Strict 1*DIGIT; `u64::from_str` would also take a leading `+`.
fn digits(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Overflowing values are still syntactically valid; saturate them.
    Some(s.parse().unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(h: &str, size: u64) -> Vec<Range<u64>> {
        match resolve(h, size) {
            Resolved::Parts(p) => p,
            other => panic!("{h}: expected parts, got {other:?}"),
        }
    }

    #[test]
    fn single_bounded_and_open_ended() {
        assert_eq!(parts("bytes=0-4", 11), vec![0..5]);
        assert_eq!(parts("bytes=6-", 11), vec![6..11]);
        assert_eq!(parts("bytes=6-999", 11), vec![6..11]);
    }

    #[test]
    fn suffix() {
        assert_eq!(parts("bytes=-5", 11), vec![6..11]);
        assert_eq!(parts("bytes=-999", 11), vec![0..11]);
        assert_eq!(resolve("bytes=-0", 11), Resolved::Unsatisfiable);
        assert_eq!(resolve("bytes=-5", 0), Resolved::Unsatisfiable);
    }

    #[test]
    fn multiple_with_whitespace() {
        assert_eq!(parts("bytes=0-1, 4-5 ,-2", 11), vec![0..2, 4..6, 9..11]);
        assert_eq!(parts("Bytes = 0-1,,3-4", 11), vec![0..2, 3..5]);
    }

    #[test]
    fn ascending_overlap_and_adjacency_coalesce() {
        assert_eq!(parts("bytes=0-4,3-7", 11), vec![0..8]);
        assert_eq!(parts("bytes=0-4,5-7", 11), vec![0..8]);
        assert_eq!(parts("bytes=0-9,2-3", 11), vec![0..10]);
        assert_eq!(parts("bytes=0-1,3-4,5-6", 11), vec![0..2, 3..7]);
    }

    #[test]
    fn out_of_order_keeps_request_order() {
        assert_eq!(parts("bytes=8-9,0-1,4-5", 11), vec![8..10, 0..2, 4..6]);
        // a range starting before the previous one is not merged into it
        assert_eq!(parts("bytes=4-5,3-4", 11), vec![4..6, 3..5]);
    }

    #[test]
    fn overlap_beyond_blob_size_is_ignored() {
        assert_eq!(resolve("bytes=0-1,3-4,-11", 11), Resolved::Full);
        assert_eq!(resolve("bytes=5-,0-", 11), Resolved::Full);
        assert_eq!(parts("bytes=0-,0-", 11), vec![0..11]);
    }

    #[test]
    fn unsatisfiable_specs_are_dropped() {
        assert_eq!(parts("bytes=99-,0-1", 11), vec![0..2]);
        assert_eq!(resolve("bytes=99-", 11), Resolved::Unsatisfiable);
        assert_eq!(resolve("bytes=11-20,50-", 11), Resolved::Unsatisfiable);
        assert_eq!(resolve("bytes=0-", 0), Resolved::Unsatisfiable);
    }

    #[test]
    fn huge_values_saturate() {
        assert_eq!(parts("bytes=0-99999999999999999999999", 11), vec![0..11]);
        assert_eq!(parts("bytes=-99999999999999999999999", 11), vec![0..11]);
    }

    #[test]
    fn malformed_or_foreign_unit_is_ignored() {
        for h in [
            "bytes=",
            "bytes=abc",
            "bytes=5-2",
            "bytes=+1-2",
            "bytes=1",
            "bytes=0-1,x",
            "items=0-1",
            "0-1",
        ] {
            assert_eq!(resolve(h, 11), Resolved::Full, "{h}");
        }
    }

    #[test]
    fn too_many_parts_is_ignored() {
        let spec: Vec<String> = (0..=MAX_PARTS as u64)
            .map(|i| format!("{}-{}", i * 2, i * 2))
            .collect();
        let h = format!("bytes={}", spec.join(","));
        assert_eq!(resolve(&h, 10_000), Resolved::Full);
        let spec: Vec<String> = (0..MAX_PARTS as u64)
            .map(|i| format!("{}-{}", i * 2, i * 2))
            .collect();
        let h = format!("bytes={}", spec.join(","));
        assert_eq!(parts(&h, 10_000).len(), MAX_PARTS);
    }
}
