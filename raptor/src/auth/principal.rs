//! Who a Management API request runs as: resolved once by `mgmt_auth` and
//! inserted as a request extension, the way `ddi_auth` inserts `AuthKind`.

/// Operator roles. Each role's hawkBit permission set is defined only in
/// [`Role::permissions`]; the method gate in `mgmt_auth` derives from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Admin,
    ReadOnly,
}

/// hawkBit 1.2 `SpPermission` groups; each gets CREATE/READ/UPDATE/DELETE.
const GROUPS: [&str; 9] = [
    "TARGET",
    "TARGET_TYPE",
    "SOFTWARE_MODULE",
    "SOFTWARE_MODULE_TYPE",
    "DISTRIBUTION_SET",
    "DISTRIBUTION_SET_TYPE",
    "ROLLOUT",
    "AUTO_ASSIGNMENT",
    "TENANT_CONFIGURATION",
];
/// hawkBit's permissions outside the CRUD grid.
const SPECIAL: [&str; 8] = [
    "READ_TARGET_SECURITY_TOKEN",
    "READ_GATEWAY_SECURITY_TOKEN",
    "READ_SOFTWARE_MODULE_ARTIFACT",
    "APPROVE_AUTO_ASSIGNMENT",
    "HANDLE_AUTO_ASSIGNMENT",
    "APPROVE_ROLLOUT",
    "HANDLE_ROLLOUT",
    "TENANT_CONFIGURATION",
];

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::ReadOnly => "read_only",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(Role::Admin),
            "read_only" => Some(Role::ReadOnly),
            _ => None,
        }
    }

    /// The role's permissions in hawkBit's vocabulary, as `/rest/v1/userinfo`
    /// reports them.
    pub fn permissions(self) -> Vec<String> {
        let all = GROUPS
            .iter()
            .flat_map(|g| ["CREATE_", "READ_", "UPDATE_", "DELETE_"].map(|p| format!("{p}{g}")))
            .chain(SPECIAL.iter().map(|s| s.to_string()));
        match self {
            Role::Admin => all.collect(),
            Role::ReadOnly => all
                .filter(|p| p.starts_with("READ_") && !p.ends_with("_SECURITY_TOKEN"))
                .collect(),
        }
    }

    /// Whether the role may do anything beyond reading.
    pub fn can_write(self) -> bool {
        self.permissions().iter().any(|p| !p.starts_with("READ_"))
    }

    /// hawkBit's `READ_TARGET_SECURITY_TOKEN`: may see target security tokens.
    pub fn sees_security_tokens(self) -> bool {
        self.permissions()
            .iter()
            .any(|p| p == "READ_TARGET_SECURITY_TOKEN")
    }
}

/// Where an identity is defined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The `[mgmt]` config account; never stored in `users`.
    Config,
    Local,
    Oidc,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Config => "config",
            Source::Local => "local",
            Source::Oidc => "oidc",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Principal {
    pub username: String,
    pub role: Role,
    pub source: Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_has_every_hawkbit_permission() {
        let p = Role::Admin.permissions();
        assert_eq!(p.len(), 9 * 4 + SPECIAL.len());
        for want in [
            "CREATE_TARGET",
            "APPROVE_ROLLOUT",
            "READ_TARGET_SECURITY_TOKEN",
        ] {
            assert!(p.iter().any(|x| x == want), "{want}");
        }
    }

    #[test]
    fn read_only_is_reads_without_tokens() {
        let p = Role::ReadOnly.permissions();
        assert!(p.iter().any(|x| x == "READ_TARGET"));
        assert!(p.iter().any(|x| x == "READ_SOFTWARE_MODULE_ARTIFACT"));
        assert!(!p.iter().any(|x| x.ends_with("_SECURITY_TOKEN")));
        assert!(!p.iter().any(|x| x == "CREATE_TARGET"));
    }

    /// The method gate and the reported permissions come from one place.
    #[test]
    fn gate_agrees_with_permissions() {
        assert!(Role::Admin.can_write() && Role::Admin.sees_security_tokens());
        assert!(!Role::ReadOnly.can_write() && !Role::ReadOnly.sees_security_tokens());
    }

    #[test]
    fn role_round_trips() {
        for r in [Role::Admin, Role::ReadOnly] {
            assert_eq!(Role::parse(r.as_str()), Some(r));
        }
        assert_eq!(Role::parse("operator"), None);
    }
}
