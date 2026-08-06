//! Table rendering for command output. `--json` bypasses this entirely and
//! prints the server's own JSON, so nothing here is a stability contract.

/// Print a simple left-aligned table: header row, then rows. Column widths
/// are computed from content so no crate is needed for `format!("{:<w$}")`.
pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |cells: &[&str], widths: &[usize]| {
        let parts: Vec<String> = cells
            .iter()
            .zip(widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect();
        println!("{}", parts.join("  ").trim_end());
    };
    line(headers, &widths);
    for row in rows {
        let refs: Vec<&str> = row.iter().map(String::as_str).collect();
        line(&refs, &widths);
    }
}

/// `Some` for a filled cell, `"-"` for absent — the convention used
/// throughout the command output so tables never have blank cells.
pub fn opt(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "-".into())
}
