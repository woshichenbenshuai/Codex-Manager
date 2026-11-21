use super::*;

struct Fixture {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}
impl Fixture {
    fn new() -> Self {
        let keys = [
            "CODEXMANAGER_DB_PATH",
            "CODEXMANAGER_STORAGE_BACKEND",
            "CODEXMANAGER_DATABASE_URL",
        ];
        let previous = keys.into_iter().map(|k| (k, std::env::var_os(k))).collect();
        let path = std::env::temp_dir().join(format!(
            "cm-2fa-{}-{}.db",
            std::process::id(),
            crypto::random_hex(8)
        ));
        std::env::set_var("CODEXMANAGER_DB_PATH", path);
        std::env::set_var("CODEXMANAGER_STORAGE_BACKEND", "sqlite");
        std::env::remove_var("CODEXMANAGER_DATABASE_URL");
        crate::storage_helpers::initialize_storage().unwrap();
        crate::set_web_access_password(Some("two-factor-test-password")).unwrap();
        crate::set_web_auth_mode("password").unwrap();
        Self { previous }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..) {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn enroll(actor: &crate::RpcActor) -> (TwoFactorSetup, TwoFactorConfirmation) {
    let setup = begin_setup(actor, "two-factor-test-password").unwrap();
    assert!(!status().unwrap().enabled);
    let code = crypto::code(&setup.secret, now_ts() / 30).unwrap();
    let confirmation =
        confirm_setup(actor, "two-factor-test-password", &setup.setup_token, &code).unwrap();
    (setup, confirmation)
}

#[test]
fn enrollment_login_recovery_and_disable_preserve_existing_password_mode() {
    let _guard = crate::test_env_guard();
    let _fixture = Fixture::new();
    let actor = crate::RpcActor::system_admin();
    assert!(!status().unwrap().enabled);
    assert!(begin_setup(&actor, "wrong").is_err());
    assert!(begin_setup(
        &crate::RpcActor::from_parts(Some("member"), Some("member")),
        "two-factor-test-password"
    )
    .is_err());
    let (setup, confirmation) = enroll(&actor);
    assert_eq!(crate::current_web_auth_mode(), "password");
    assert_eq!(confirmation.recovery_codes.len(), 8);
    assert!(!storage::load().unwrap().state_json.contains(&setup.secret));
    assert!(!storage::load()
        .unwrap()
        .state_json
        .contains(&confirmation.recovery_codes[0]));
    assert!(crate::set_web_auth_mode("none").is_err());
    assert!(!receipt_valid("password", "primary", "").unwrap());

    let ticket = issue_login("password", "primary", None).unwrap().unwrap();
    let code = crypto::code(&setup.secret, now_ts() / 30).unwrap();
    assert!(
        complete_login(&ticket, &code).is_err(),
        "enrollment already consumed this time step"
    );
    let login = complete_login(&ticket, &confirmation.recovery_codes[0]).unwrap();
    assert_eq!(login.token, "primary");
    assert!(receipt_valid("password", "primary", &login.receipt).unwrap());
    assert!(!receipt_valid("password", "another", &login.receipt).unwrap());
    assert!(!receipt_valid("accounts", "primary", &login.receipt).unwrap());
    assert!(complete_login(&ticket, &confirmation.recovery_codes[1]).is_err());
    let second = issue_login("password", "primary", None).unwrap().unwrap();
    assert!(complete_login(&second, &confirmation.recovery_codes[0]).is_err());
    assert_eq!(status().unwrap().recovery_codes_remaining, 7);
    assert!(disable(&actor, "wrong", &confirmation.recovery_codes[1]).is_err());
    disable(
        &actor,
        "two-factor-test-password",
        &confirmation.recovery_codes[1],
    )
    .unwrap();
    assert!(!status().unwrap().enabled);
    assert!(complete_login(&second, &confirmation.recovery_codes[2]).is_err());
    assert_eq!(crate::current_web_auth_mode(), "password");
}

#[test]
fn expired_and_exhausted_challenges_cannot_complete_or_replay() {
    let _guard = crate::test_env_guard();
    let _fixture = Fixture::new();
    let actor = crate::RpcActor::system_admin();
    let (_, confirmation) = enroll(&actor);
    let ticket = issue_login("password", "primary", None).unwrap().unwrap();
    for _ in 0..5 {
        assert!(complete_login(&ticket, "wrong").is_err());
    }
    assert!(complete_login(&ticket, &confirmation.recovery_codes[0]).is_err());
    let expired = issue_login("password", "primary", None).unwrap().unwrap();
    update(|s| {
        s.tickets.iter_mut().for_each(|t| t.expires = 0);
        Ok(())
    })
    .unwrap();
    assert!(complete_login(&expired, &confirmation.recovery_codes[0]).is_err());
    assert_eq!(status().unwrap().recovery_codes_remaining, 8);
}

#[test]
fn accounts_login_keeps_account_credentials_and_requires_the_second_factor() {
    let _guard = crate::test_env_guard();
    let _fixture = Fixture::new();
    crate::set_web_auth_mode("accounts").unwrap();
    let login = crate::bootstrap_app_admin("operator", "two-factor-test-password", None).unwrap();
    let actor = crate::RpcActor::from_parts(Some("admin"), Some(&login.user.id));
    let (_, confirmation) = enroll(&actor);
    let pending = issue_login("accounts", &login.token, Some(login.user.id.clone()))
        .unwrap()
        .unwrap();
    let completed = complete_login(&pending, &confirmation.recovery_codes[0]).unwrap();
    assert!(crate::resolve_app_user_session(&completed.token)
        .unwrap()
        .is_some());
    assert!(receipt_valid("accounts", &completed.token, &completed.receipt).unwrap());
    assert_eq!(crate::current_web_auth_mode(), "accounts");
}
