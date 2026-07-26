//! Authentication middleware: `ddi` (gateway/target token) and `mgmt`
//! (Basic auth or session cookie), plus the in-memory `session` store backing
//! the mgmt UI's cookie session.

pub mod ddi;
pub mod mgmt;
pub mod session;
