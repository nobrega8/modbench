//! Firmware version strings and ranges (SPEC.md section 3.2 of
//! mbmap-library): whole numbers separated by dots, compared as numbers from
//! the left, where a bound covers every version that starts with it.

use std::cmp::Ordering;

/// A version as numbers, e.g. `2.01` -> `[2, 1]`.
pub type Version = Vec<u64>;

/// A firmware range; either side is open when `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Range {
    pub min: Option<Version>,
    pub max: Option<Version>,
}

/// Parses a version string written in a map: `"2"`, `"2.1"`, `"1.02.7"`.
pub fn parse(text: &str) -> Option<Version> {
    if text.is_empty() {
        return None;
    }
    text.split('.')
        .map(|part| {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        })
        .collect()
}

/// The first version string in a device's text, e.g. `"V2.01b"` -> `2.01`.
pub fn find(text: &str) -> Option<Version> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    parse(rest[..end].trim_end_matches('.'))
}

/// Compares `v` with a bound on the components the bound gives (missing
/// components of `v` count as 0), so a bound covers versions that start with it.
fn cmp_to_bound(v: &[u64], bound: &[u64]) -> Ordering {
    for (i, b) in bound.iter().enumerate() {
        match v.get(i).copied().unwrap_or(0).cmp(b) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Whether every version up to `max` is older than `min`. `max` covers the
/// versions that start with it, so `max` "2" still reaches `min` "2.1".
pub fn max_before_min(max: &[u64], min: &[u64]) -> bool {
    for (i, h) in max.iter().enumerate() {
        let low = min.get(i).copied().unwrap_or(0);
        if *h != low {
            return *h < low;
        }
    }
    false
}

impl Range {
    /// Whether the range is open on both sides (applies to every firmware).
    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// Whether `min` comes after `max`.
    pub fn is_inverted(&self) -> bool {
        matches!((&self.min, &self.max), (Some(lo), Some(hi)) if max_before_min(hi, lo))
    }

    /// Whether a device version is inside the range.
    pub fn contains(&self, v: &[u64]) -> bool {
        self.min
            .as_ref()
            .is_none_or(|lo| cmp_to_bound(v, lo) != Ordering::Less)
            && self
                .max
                .as_ref()
                .is_none_or(|hi| cmp_to_bound(v, hi) != Ordering::Greater)
    }

    /// Whether two ranges have a version in common.
    pub fn meets(&self, other: &Range) -> bool {
        let apart = |a: &Range, b: &Range| matches!((&a.max, &b.min), (Some(hi), Some(lo)) if max_before_min(hi, lo));
        !apart(self, other) && !apart(other, self)
    }
}

/// A version written back as text: `[2, 1]` -> `"2.1"`.
pub fn display(v: &[u64]) -> String {
    v.iter().map(u64::to_string).collect::<Vec<_>>().join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: Option<&str>, max: Option<&str>) -> Range {
        Range {
            min: min.and_then(parse),
            max: max.and_then(parse),
        }
    }

    #[test]
    fn parses_and_finds_versions() {
        assert_eq!(parse("1.02.7"), Some(vec![1, 2, 7]));
        assert_eq!(parse("2"), Some(vec![2]));
        for bad in ["", "v1", "2.x", "1..2", "1.", ".1", "1.2 "] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        assert_eq!(find("V2.01b"), Some(vec![2, 1]));
        assert_eq!(find("fw 3."), Some(vec![3]));
        assert_eq!(find("none"), None);
    }

    #[test]
    fn bounds_cover_versions_that_start_with_them() {
        let r = range(Some("2.1"), Some("2"));
        assert!(r.contains(&[2, 1]) && r.contains(&[2, 1, 4]) && r.contains(&[2, 9]));
        assert!(!r.contains(&[2, 0, 9]) && !r.contains(&[3, 0]));
        let r = range(None, Some("2.3"));
        assert!(r.contains(&[2, 3, 7]) && !r.contains(&[2, 4]));
        // Components compare as numbers.
        assert!(range(Some("1.9"), None).contains(&[1, 10]));
        assert!(range(None, None).contains(&[0]));
    }

    #[test]
    fn inverted_and_meeting_ranges() {
        assert!(range(Some("3"), Some("2.9")).is_inverted());
        assert!(range(Some("2.2"), Some("2.1")).is_inverted());
        assert!(!range(Some("2.1"), Some("2")).is_inverted());
        assert!(!range(Some("2.0"), Some("2.0")).is_inverted());

        let old = range(None, Some("2.0"));
        let new = range(Some("2.1"), None);
        assert!(!old.meets(&new) && !new.meets(&old));
        assert!(range(None, Some("2.4")).meets(&range(Some("2.4"), None)));
        assert!(range(None, Some("2")).meets(&range(Some("2.1"), None)));
        assert!(!range(None, Some("1.9")).meets(&range(Some("2"), Some("3"))));
        assert!(range(Some("3"), None).meets(&range(Some("2"), Some("3"))));
    }
}
