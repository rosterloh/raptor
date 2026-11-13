//! Authentication middleware: `ddi` (gateway/target token) and `mgmt`
//! (session cookie, Basic auth or OIDC bearer), plus the in-memory `session` store backing
//! the mgmt UI's cookie session.

pub mod ddi;
pub mod mgmt;
pub mod oidc;
pub mod principal;
pub mod session;
