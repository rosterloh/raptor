//! HTTP layer: the `ddi` (device) and `mgmt` (management) route trees, plus
//! shared paging helpers. Handlers only — business rules live in `domain/`.

pub mod ddi;
pub mod mgmt;
pub mod paging;
