use serde::Serialize;

pub const ROLE_SYSTEM_ADMIN: &str = "system_admin";
pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_MEMBER: &str = "member";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcActor {
    pub role: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
}

impl RpcActor {
    pub fn system_admin() -> Self {
        Self {
            role: ROLE_SYSTEM_ADMIN.to_string(),
            user_id: None,
            session_id: None,
        }
    }

    pub fn from_parts(role: Option<&str>, user_id: Option<&str>) -> Self {
        Self::from_parts_with_session(role, user_id, None)
    }

    pub fn from_parts_with_session(
        role: Option<&str>,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) -> Self {
        let normalized_role = normalize_role(role);
        Self {
            role: normalized_role.to_string(),
            user_id: user_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            session_id: session_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role.as_str(), ROLE_SYSTEM_ADMIN | ROLE_ADMIN)
    }

    pub fn is_member(&self) -> bool {
        self.role == ROLE_MEMBER
    }

    pub fn permissions(&self) -> Vec<&'static str> {
        if self.is_admin() {
            return vec!["system:admin"];
        }
        vec![
            "apikey:self",
            "requestlog:self",
            "models:read",
            "profile:self",
        ]
    }
}

fn normalize_role(role: Option<&str>) -> &'static str {
    let Some(role) = role.map(str::trim).filter(|value| !value.is_empty()) else {
        // No actor headers are the trusted local/system-admin path. Once a
        // role is supplied, unknown legacy values must fail closed as member.
        return ROLE_SYSTEM_ADMIN;
    };
    match role.to_ascii_lowercase().as_str() {
        ROLE_ADMIN => ROLE_ADMIN,
        ROLE_MEMBER => ROLE_MEMBER,
        ROLE_SYSTEM_ADMIN => ROLE_SYSTEM_ADMIN,
        "operator" => ROLE_MEMBER,
        _ => ROLE_MEMBER,
    }
}

#[cfg(test)]
mod tests {
    use super::{RpcActor, ROLE_MEMBER, ROLE_SYSTEM_ADMIN};

    #[test]
    fn missing_actor_headers_keep_local_system_admin_path() {
        assert_eq!(RpcActor::from_parts(None, None).role, ROLE_SYSTEM_ADMIN);
    }

    #[test]
    fn unknown_or_legacy_user_roles_fail_closed_as_member() {
        assert_eq!(
            RpcActor::from_parts(Some("operator"), Some("u")).role,
            ROLE_MEMBER
        );
        assert_eq!(
            RpcActor::from_parts(Some("forged"), Some("u")).role,
            ROLE_MEMBER
        );
    }
}
