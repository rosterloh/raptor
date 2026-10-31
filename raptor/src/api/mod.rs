//! HTTP layer: the `ddi` (device) and `mgmt` (management) route trees, plus
//! shared paging helpers. Handlers only — business rules live in `domain/`.

pub mod ddi;
pub mod mgmt;
pub mod paging;

/// `Content-Disposition` for an artifact download. Names stored before upload
/// validation (#183) may hold bytes a header can't carry; replace them so the
/// download still works instead of failing to build the response.
pub(crate) fn attachment(filename: &str) -> String {
    let safe: String = filename
        .chars()
        .map(|c| {
            if c.is_control() || c == '"' || c == '\\' {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("attachment; filename=\"{safe}\"")
}
