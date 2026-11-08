//! `tailwind.css` declares each theme twice (dark in `@theme` and `.theme-dark`,
//! light in the media query and `.theme-light`), because CSS has no "same
//! values, different trigger". These keep each pair identical, so a token edited
//! in one place can't leave the operator's explicit choice on the old value.

use std::collections::BTreeMap;

const CSS: &str = include_str!("../tailwind.css");

/// `--color-*` declarations in the first `{ … }` after `marker`.
fn colours(marker: &str) -> BTreeMap<&'static str, &'static str> {
    let start = CSS.find(marker).expect("marker present in tailwind.css") + marker.len();
    let body = &CSS[start..];
    let body = &body[body.find('{').expect("block opens") + 1..];
    let body = &body[..body.find('}').expect("block closes")];
    body.lines()
        .filter_map(|l| l.trim().strip_prefix("--color-"))
        .filter_map(|l| l.trim_end_matches(';').split_once(':'))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect()
}

#[test]
fn dark_theme_blocks_match() {
    let theme = colours("@theme");
    assert!(theme.len() > 30, "parsed {} tokens", theme.len());
    assert_eq!(theme, colours(":root.theme-dark"));
}

#[test]
fn light_theme_blocks_match() {
    let media = colours("@media (prefers-color-scheme: light)");
    assert!(media.len() > 30, "parsed {} tokens", media.len());
    assert_eq!(media, colours(":root.theme-light"));
}
