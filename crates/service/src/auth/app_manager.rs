use argon2::{
    password_hash::{rand_core::OsRng as PasswordOsRng, PasswordHash, PasswordHasher, SaltString},
    Argon2, PasswordVerifier,
};
use codexmanager_core::storage::{
    now_ts, ApiKeyOwner, AppLoginChallenge, AppUser, AppUserAccessSummary, AppUserSession,
    AppWallet, AppWalletLedgerEntry, BillingRule, PublicAppUserWithWallet, Storage,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

use crate::app_settings::{
    get_persisted_app_setting, normalize_optional_text, parse_bool_with_default,
    save_persisted_app_setting, APP_SETTING_DISTRIBUTION_ENABLED_KEY,
    APP_SETTING_WEB_AUTH_MODE_KEY,
};
use crate::storage_helpers::open_storage;
use crate::RpcActor;

pub const WEB_AUTH_MODE_NONE: &str = "none";
pub const WEB_AUTH_MODE_PASSWORD: &str = "password";
pub const WEB_AUTH_MODE_ACCOUNTS: &str = "accounts";
const SESSION_TTL_SECONDS: i64 = 60 * 60 * 24 * 14;
const LOGIN_CHALLENGE_PURPOSE: &str = "login";
const SETUP_CHALLENGE_PURPOSE: &str = "totp_setup";
const LOGIN_SETUP_CHALLENGE_PURPOSE: &str = "totp_login_setup";
const BOOTSTRAP_SETUP_CHALLENGE_PURPOSE: &str = "bootstrap_totp_setup";
const MAX_PASSWORD_BYTES: usize = 256;
const AUTH_THROTTLED_ERROR: &str = "登录尝试过于频繁，请稍后再试";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserPublicResult {
    pub id: String,
    pub username: String,
    pub display_name: Option<String>,
    pub role: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_login_at: Option<i64>,
    pub totp_enabled: bool,
    pub totp_confirmed_at: Option<i64>,
    pub wallet: Option<AppWalletResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppWalletResult {
    pub id: String,
    pub owner_kind: String,
    pub owner_id: String,
    pub balance_credit_micros: i64,
    pub frozen_credit_micros: i64,
    pub available_credit_micros: i64,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLoginResult {
    pub token: String,
    pub expires_at: i64,
    pub user: AppUserPublicResult,
}

#[derive(Clone)]
pub enum AppLoginAttempt {
    Authenticated(AppLoginResult),
    TotpChallenge {
        challenge_token: String,
        user: AppUserPublicResult,
        error: Option<String>,
    },
    TotpSetup {
        challenge_token: String,
        setup: AppUserTotpSetupResult,
    },
}

impl std::fmt::Debug for AppLoginAttempt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authenticated(result) => f
                .debug_tuple("Authenticated")
                .field(&format_args!("user_id={}", result.user.id))
                .finish(),
            Self::TotpChallenge { user, error, .. } => f
                .debug_struct("TotpChallenge")
                .field("challenge_token", &"[REDACTED]")
                .field("user_id", &user.id)
                .field("error", error)
                .finish(),
            Self::TotpSetup { setup, .. } => f
                .debug_struct("TotpSetup")
                .field("challenge_token", &"[REDACTED]")
                .field("user_id", &setup.user_id)
                .finish(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserTotpSetupResult {
    pub user_id: String,
    pub username: String,
    pub secret: String,
    pub otpauth_uri: String,
    pub challenge_token: Option<String>,
}

impl std::fmt::Debug for AppUserTotpSetupResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppUserTotpSetupResult")
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("secret", &"[REDACTED]")
            .field("otpauth_uri", &"[REDACTED]")
            .field(
                "challenge_token",
                &self.challenge_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserTotpStatusResult {
    pub enabled: bool,
    pub confirmed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSessionUserResult {
    pub session_id: String,
    pub expires_at: i64,
    pub user: AppUserPublicResult,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserSessionResult {
    pub session_id: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub expires_at: i64,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyOwnerResult {
    pub key_id: String,
    pub owner_kind: String,
    pub owner_user_id: Option<String>,
    pub project_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSessionResult {
    pub mode: String,
    pub current_user: Option<AppUserPublicResult>,
    pub role: String,
    pub permissions: Vec<String>,
    pub distribution_enabled: bool,
    pub billing_mode_lock: BillingModeLockResult,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingModeLockResult {
    pub account_mode_locked: bool,
    pub distribution_locked: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserCreateInput {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub initial_balance_credit_micros: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserCreateResult {
    pub user: AppUserPublicResult,
    pub totp_setup: Option<AppUserTotpSetupResult>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUserUpdateInput {
    pub id: String,
    pub display_name: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
    pub password: Option<String>,
}

pub fn current_web_auth_mode() -> String {
    if let Some(raw) = get_persisted_app_setting(APP_SETTING_WEB_AUTH_MODE_KEY) {
        let mode = normalize_web_auth_mode(Some(&raw));
        // Once an active administrator exists, account authentication is the
        // only valid Web authentication mode. This also protects upgraded
        // databases whose persisted setting still says `none`.
        if mode != WEB_AUTH_MODE_ACCOUNTS && active_admin_exists() {
            return WEB_AUTH_MODE_ACCOUNTS.to_string();
        }
        if mode == WEB_AUTH_MODE_PASSWORD && !super::web_access::web_access_password_configured() {
            return WEB_AUTH_MODE_NONE.to_string();
        }
        return mode.to_string();
    }
    if active_admin_exists() {
        return WEB_AUTH_MODE_ACCOUNTS.to_string();
    }
    if super::web_access::web_access_password_configured() {
        WEB_AUTH_MODE_PASSWORD.to_string()
    } else {
        WEB_AUTH_MODE_NONE.to_string()
    }
}

pub fn set_web_auth_mode(mode: &str) -> Result<String, String> {
    let normalized = normalize_web_auth_mode(Some(mode));
    if normalized == WEB_AUTH_MODE_ACCOUNTS && active_admin_exists() {
        super::totp::validate_encryption_key()?;
    }
    let current = current_web_auth_mode();
    if current == WEB_AUTH_MODE_ACCOUNTS && normalized != WEB_AUTH_MODE_ACCOUNTS {
        let lock = billing_mode_lock_status()?;
        if lock.account_mode_locked {
            return Err("account_billing_mode_locked".to_string());
        }
    }
    if normalized != WEB_AUTH_MODE_ACCOUNTS && active_admin_exists() {
        return Err("已有管理员账号，不能关闭账户登录模式".to_string());
    }
    if normalized == WEB_AUTH_MODE_PASSWORD && !super::web_access::web_access_password_configured()
    {
        return Err("启用访问密码模式前需要先设置访问密码".to_string());
    }
    if normalized == WEB_AUTH_MODE_PASSWORD {
        return Err("独立访问密码模式已废弃，请使用账户登录模式".to_string());
    }
    save_persisted_app_setting(APP_SETTING_WEB_AUTH_MODE_KEY, Some(normalized))?;
    Ok(normalized.to_string())
}

pub fn distribution_enabled() -> bool {
    get_persisted_app_setting(APP_SETTING_DISTRIBUTION_ENABLED_KEY)
        .as_deref()
        .map(|raw| parse_bool_with_default(raw, false))
        .unwrap_or(false)
}

pub(crate) fn distribution_enabled_for_storage(storage: &Storage) -> bool {
    let raw = storage
        .get_app_setting(APP_SETTING_DISTRIBUTION_ENABLED_KEY)
        .ok()
        .flatten();
    normalize_optional_text(raw.as_deref())
        .as_deref()
        .map(|raw| parse_bool_with_default(raw, false))
        .unwrap_or(false)
}

pub fn set_distribution_enabled(enabled: bool) -> Result<bool, String> {
    if enabled && current_web_auth_mode() != WEB_AUTH_MODE_ACCOUNTS {
        return Err("distribution_requires_accounts_mode".to_string());
    }
    if !enabled && distribution_enabled() {
        let lock = billing_mode_lock_status()?;
        if lock.distribution_locked {
            return Err("distribution_mode_locked".to_string());
        }
    }
    save_persisted_app_setting(
        APP_SETTING_DISTRIBUTION_ENABLED_KEY,
        Some(if enabled { "true" } else { "false" }),
    )?;
    Ok(enabled)
}

pub fn billing_mode_lock_status() -> Result<BillingModeLockResult, String> {
    let storage = open_storage_or_error()?;
    billing_mode_lock_status_for_storage(&storage)
}

fn billing_mode_lock_status_for_storage(
    storage: &Storage,
) -> Result<BillingModeLockResult, String> {
    let reasons = billing_mode_lock_reasons(storage)?;
    let has_reasons = !reasons.is_empty();
    Ok(BillingModeLockResult {
        account_mode_locked: has_reasons,
        distribution_locked: has_reasons,
        reasons,
    })
}

fn billing_mode_lock_reasons(storage: &Storage) -> Result<Vec<String>, String> {
    let mut reasons = Vec::new();
    if storage
        .member_app_user_count()
        .map_err(|err| format!("read member users failed: {err}"))?
        > 0
    {
        reasons.push("member_users".to_string());
    }
    if storage
        .api_key_owner_count()
        .map_err(|err| format!("read api key owners failed: {err}"))?
        > 0
    {
        reasons.push("api_key_owners".to_string());
    }
    if storage
        .nonzero_wallet_count()
        .map_err(|err| format!("read wallets failed: {err}"))?
        > 0
    {
        reasons.push("wallet_balance".to_string());
    }
    if storage
        .wallet_ledger_entry_count()
        .map_err(|err| format!("read wallet ledger failed: {err}"))?
        > 0
    {
        reasons.push("wallet_ledger".to_string());
    }
    if storage
        .user_model_group_assignment_count()
        .map_err(|err| format!("read model group assignments failed: {err}"))?
        > 0
    {
        reasons.push("model_group_assignments".to_string());
    }
    if storage
        .request_charge_ledger_entry_count()
        .map_err(|err| format!("read wallet request charges failed: {err}"))?
        > 0
    {
        reasons.push("request_charges".to_string());
    }
    Ok(reasons)
}

pub fn app_auth_status_value() -> Result<Value, String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let user_count = storage
        .app_user_count()
        .map_err(|err| format!("read app users failed: {err}"))?;
    let active_admin_count = storage
        .active_admin_count()
        .map_err(|err| format!("read app admins failed: {err}"))?;
    let billing_mode_lock = billing_mode_lock_status_for_storage(&storage)?;
    Ok(serde_json::json!({
        "mode": current_web_auth_mode(),
        "modeOptions": [
            WEB_AUTH_MODE_NONE,
            WEB_AUTH_MODE_ACCOUNTS
        ],
        "passwordConfigured": super::web_access::web_access_password_configured(),
        "appUsersConfigured": active_admin_count > 0,
        "appUserCount": user_count,
        "activeAdminCount": active_admin_count,
        "distributionEnabled": distribution_enabled(),
        "billingModeLock": billing_mode_lock,
    }))
}

pub fn app_session_result(actor: &RpcActor) -> Result<AppSessionResult, String> {
    crate::initialize_storage_if_needed()?;
    let current_user = actor
        .user_id
        .as_deref()
        .map(|user_id| {
            let storage = open_storage_or_error()?;
            let user = storage
                .find_public_app_user_with_wallet_by_id(user_id)
                .map_err(|err| format!("read app user failed: {err}"))?
                .ok_or_else(|| "当前用户不存在".to_string())?;
            let mut result = public_user_with_wallet(user);
            fill_totp_state(&storage, &mut result)?;
            Ok::<_, String>(result)
        })
        .transpose()?;
    let billing_mode_lock = if actor.is_admin() {
        billing_mode_lock_status()?
    } else {
        BillingModeLockResult::default()
    };
    Ok(AppSessionResult {
        mode: current_web_auth_mode(),
        current_user,
        role: actor.role.clone(),
        permissions: actor
            .permissions()
            .into_iter()
            .map(str::to_string)
            .collect(),
        distribution_enabled: distribution_enabled(),
        billing_mode_lock,
    })
}

pub fn bootstrap_app_admin(
    username: &str,
    password: &str,
    display_name: Option<&str>,
) -> Result<AppLoginResult, String> {
    let _ = (username, password, display_name);
    Err("管理员初始化必须完成验证器绑定，请使用 bootstrap_app_admin_with_totp".to_string())
}

pub fn login_app_user(
    username: &str,
    password: &str,
    totp_code: Option<&str>,
    challenge_token: Option<&str>,
) -> Result<AppLoginAttempt, String> {
    login_app_user_from_source(username, password, totp_code, challenge_token, None)
}

pub fn validate_web_totp_configuration() -> Result<(), String> {
    super::totp::validate_encryption_key()?;
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    for ciphertext in storage
        .list_enabled_app_user_totp_ciphertexts()
        .map_err(|err| format!("read enabled authenticator keys failed: {err}"))?
    {
        super::totp::decrypt_secret(&ciphertext)?;
    }
    Ok(())
}

pub fn login_app_user_from_source(
    username: &str,
    password: &str,
    totp_code: Option<&str>,
    challenge_token: Option<&str>,
    source_ip: Option<&str>,
) -> Result<AppLoginAttempt, String> {
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    if let Some(challenge_token) = challenge_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return complete_app_login_challenge_with_storage(&mut storage, challenge_token, totp_code);
    }
    let username_subject =
        throttle_subject("username", username.trim().to_ascii_lowercase().as_bytes());
    let source_subject = source_ip
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| throttle_subject("source-ip", value.as_bytes()));
    if let Some(subject) = source_subject.as_deref() {
        reserve_auth_attempt_or_throttled(
            &mut storage,
            "login_source_ip",
            subject,
            300,
            30,
            300,
            300,
        )?;
    }
    reserve_auth_attempt_or_throttled(
        &mut storage,
        "login_username",
        &username_subject,
        86_400,
        5,
        30,
        900,
    )?;

    let normalized_username = normalize_username(username).ok();
    let user = normalized_username
        .as_deref()
        .map(|normalized| {
            storage
                .find_app_user_by_username(normalized)
                .map_err(|err| format!("read app user failed: {err}"))
        })
        .transpose()?
        .flatten();
    let password_valid_shape = password.as_bytes().len() <= MAX_PASSWORD_BYTES;
    let password_matches = if let Some(user) = user
        .as_ref()
        .filter(|user| password_valid_shape && user.status == "active")
    {
        verify_password_hash_guarded(password, &user.password_hash)?
    } else {
        verify_dummy_password_guarded()?;
        false
    };
    let Some(user) = user else {
        return Err("用户名或密码错误".to_string());
    };
    if user.status != "active" || !password_matches {
        return Err("用户名或密码错误".to_string());
    }
    storage
        .clear_auth_throttle("login_username", &username_subject)
        .map_err(|err| format!("clear login throttle failed: {err}"))?;
    if let Some(subject) = source_subject.as_deref() {
        storage
            .clear_auth_throttle("login_source_ip", subject)
            .map_err(|err| format!("clear source login throttle failed: {err}"))?;
    }
    if !user.password_hash.starts_with("$argon2") {
        let next_hash = hash_password_guarded(password)?;
        storage
            .update_app_user_password_hash(&user.id, &next_hash)
            .map_err(|err| format!("upgrade app user password hash failed: {err}"))?;
    }
    let totp = storage
        .find_app_user_totp_state(&user.id)
        .map_err(|err| format!("read app user authenticator failed: {err}"))?
        .ok_or_else(|| "用户认证配置缺失".to_string())?;
    if user.role == "admin" && !totp.enabled {
        let setup = begin_totp_setup_with_storage(
            &mut storage,
            &user,
            &user.id,
            LOGIN_SETUP_CHALLENGE_PURPOSE,
        )?;
        let challenge_token = setup.challenge_token.clone().unwrap_or_default();
        return Ok(AppLoginAttempt::TotpSetup {
            challenge_token,
            setup,
        });
    }
    if totp.enabled {
        let challenge_token = create_login_challenge(
            &mut storage,
            &user.id,
            &user.id,
            LOGIN_CHALLENGE_PURPOSE,
            None,
        )?;
        if totp_code
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        {
            return match complete_app_login_challenge_with_storage(
                &mut storage,
                &challenge_token,
                totp_code,
            ) {
                Ok(attempt) => Ok(attempt),
                Err(err) => Ok(AppLoginAttempt::TotpChallenge {
                    challenge_token,
                    user: public_user_with_totp_state(user, None, &totp),
                    error: Some(err),
                }),
            };
        } else {
            return Ok(AppLoginAttempt::TotpChallenge {
                challenge_token,
                user: public_user_with_totp_state(user, None, &totp),
                error: None,
            });
        }
    }
    let now = now_ts();
    storage
        .update_app_user_last_login(&user.id, now)
        .map_err(|err| format!("update app user login failed: {err}"))?;
    let mut next = user;
    next.last_login_at = Some(now);
    next.updated_at = now;
    Ok(AppLoginAttempt::Authenticated(create_session_with_storage(
        &storage, next,
    )?))
}

fn active_admin_exists() -> bool {
    crate::initialize_storage_if_needed()
        .ok()
        .and_then(|_| open_storage_or_error().ok())
        .and_then(|storage| storage.active_admin_count().ok())
        .is_some_and(|count| count > 0)
}

pub fn complete_app_login_challenge(
    challenge_token: &str,
    totp_code: &str,
) -> Result<AppLoginResult, String> {
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    match complete_app_login_challenge_with_storage(&mut storage, challenge_token, Some(totp_code))?
    {
        AppLoginAttempt::Authenticated(result) => Ok(result),
        _ => Err("登录挑战未完成".to_string()),
    }
}

pub fn confirm_app_user_totp_setup(
    actor: Option<&RpcActor>,
    challenge_token: Option<&str>,
    totp_code: &str,
) -> Result<AppLoginResult, String> {
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    if let Some(token) = challenge_token
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let purpose = find_setup_challenge_purpose(&storage, token)?;
        if !matches!(
            purpose.as_str(),
            LOGIN_SETUP_CHALLENGE_PURPOSE | BOOTSTRAP_SETUP_CHALLENGE_PURPOSE
        ) {
            return Err("绑定挑战无效，请重新开始绑定".to_string());
        }
        let challenge = reserve_login_challenge(&mut storage, token, &purpose)?;
        let mut user = storage
            .find_app_user_by_id(&challenge.target_user_id)
            .map_err(|err| format!("read app user failed: {err}"))?
            .ok_or_else(|| "用户不存在".to_string())?;
        if user.status != "active" {
            return Err("绑定挑战已失效，请重新开始绑定".to_string());
        }
        let now = now_ts();
        let (session_token, session) = new_session(&user, now);
        let finalize_legacy_migration = purpose == BOOTSTRAP_SETUP_CHALLENGE_PURPOSE
            || (purpose == LOGIN_SETUP_CHALLENGE_PURPOSE
                && user.role == "admin"
                && super::web_access::web_access_password_configured());
        confirm_totp_setup_with_storage(
            &mut storage,
            &challenge,
            &user,
            totp_code,
            Some(&session),
            None,
            finalize_legacy_migration,
        )?;
        user.last_login_at = Some(now);
        user.updated_at = now;
        return session_result_with_storage(&storage, user, session_token, &session);
    }
    let _ = actor;
    Err("绑定挑战缺失，请重新开始绑定".to_string())
}

pub fn confirm_app_user_totp_setup_for_actor(
    actor: &RpcActor,
    target_user_id: &str,
    challenge_token: &str,
    totp_code: &str,
) -> Result<AppUserTotpStatusResult, String> {
    let actor_user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "permission_denied: authenticator setup requires user session".to_string()
        })?;
    let challenge_token = challenge_token.trim();
    if challenge_token.is_empty() {
        return Err("绑定挑战缺失，请重新开始绑定".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    let target_user_id = target_user_id.trim();
    if target_user_id.is_empty() {
        return Err("目标用户 ID 不能为空".to_string());
    }
    if target_user_id != actor_user_id && !actor.is_admin() {
        return Err(
            "permission_denied: only admins may confirm another user authenticator".to_string(),
        );
    }
    let user = storage
        .find_app_user_by_id(target_user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    if user.status != "active" {
        return Err("当前账号不可用".to_string());
    }
    let challenge =
        reserve_login_challenge(&mut storage, challenge_token, SETUP_CHALLENGE_PURPOSE)?;
    if challenge.target_user_id != target_user_id
        || challenge.initiated_by_user_id.as_deref() != Some(actor_user_id)
    {
        return Err("绑定挑战无效，请重新开始绑定".to_string());
    }
    confirm_totp_setup_with_storage(
        &mut storage,
        &challenge,
        &user,
        totp_code,
        None,
        (target_user_id == actor_user_id)
            .then_some(actor.session_id.as_deref())
            .flatten(),
        false,
    )?;
    Ok(AppUserTotpStatusResult {
        enabled: true,
        confirmed_at: Some(now_ts()),
    })
}

pub fn bootstrap_app_admin_with_totp(
    bootstrap_password: &str,
    username: &str,
    password: &str,
    display_name: Option<&str>,
) -> Result<AppLoginAttempt, String> {
    crate::initialize_storage_if_needed()?;
    super::totp::validate_encryption_key()?;
    let mut storage = open_storage_or_error()?;
    if storage
        .active_admin_count()
        .map_err(|err| format!("read app admins failed: {err}"))?
        > 0
    {
        return Err("管理员已初始化".to_string());
    }
    let bootstrap_subject = throttle_subject("bootstrap", b"first-admin");
    reserve_auth_attempt_or_throttled(
        &mut storage,
        "bootstrap",
        &bootstrap_subject,
        300,
        5,
        30,
        900,
    )?;
    if !super::web_access::web_access_password_configured()
        || !super::web_access::verify_web_access_password(bootstrap_password)
    {
        return Err("旧访问密码错误".to_string());
    }
    storage
        .clear_auth_throttle("bootstrap", &bootstrap_subject)
        .map_err(|err| format!("clear bootstrap throttle failed: {err}"))?;
    let username = normalize_username(username)?;
    validate_password(password)?;
    let now = now_ts();
    let user = AppUser {
        id: generate_id("usr", 8),
        username,
        display_name: normalize_optional_text(display_name),
        password_hash: hash_password_guarded(password)?,
        role: "admin".to_string(),
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
        last_login_at: None,
    };
    let secret = super::totp::generate_secret();
    let challenge_token = format!("cmc_{}", random_hex(32));
    let challenge = AppLoginChallenge {
        id: generate_id("challenge", 8),
        user_id: user.id.clone(),
        token_hash: token_hash(&challenge_token),
        purpose: BOOTSTRAP_SETUP_CHALLENGE_PURPOSE.to_string(),
        expires_at: now.saturating_add(300),
        attempts: 0,
        last_attempt_at: None,
        used_at: None,
        created_at: now,
        setup_secret_ciphertext: Some(super::totp::encrypt_secret(&secret)?),
        initiated_by_user_id: None,
        target_user_id: user.id.clone(),
        locked_until: None,
    };
    if !storage
        .claim_first_app_admin_with_challenge(&user, &challenge)
        .map_err(|err| format!("create first administrator failed: {err}"))?
    {
        return Err("管理员已初始化".to_string());
    }
    let setup = AppUserTotpSetupResult {
        user_id: user.id.clone(),
        username: user.username.clone(),
        secret: secret.clone(),
        otpauth_uri: super::totp::otpauth_uri(&user.username, &secret),
        challenge_token: Some(challenge_token.clone()),
    };
    Ok(AppLoginAttempt::TotpSetup {
        challenge_token,
        setup,
    })
}

pub fn list_app_user_sessions(actor: &RpcActor) -> Result<Vec<AppUserSessionResult>, String> {
    let user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "permission_denied: session list requires user session".to_string())?;
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    storage
        .list_app_user_sessions(user_id)
        .map_err(|err| format!("list app user sessions failed: {err}"))
        .map(|sessions| {
            sessions
                .into_iter()
                .filter(|session| session.revoked_at.is_none())
                .map(|session| AppUserSessionResult {
                    current: actor.session_id.as_deref() == Some(session.id.as_str()),
                    session_id: session.id,
                    created_at: session.created_at,
                    last_seen_at: session.last_seen_at,
                    expires_at: session.expires_at,
                })
                .collect()
        })
}

pub fn revoke_app_user_session(actor: &RpcActor, session_id: &str) -> Result<(), String> {
    let user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "permission_denied: session revoke requires user session".to_string())?;
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    if !storage
        .revoke_app_user_session(session_id, user_id, now_ts())
        .map_err(|err| format!("revoke app user session failed: {err}"))?
    {
        return Err("会话不存在或已失效".to_string());
    }
    Ok(())
}

pub fn begin_app_user_totp_setup(
    actor: &RpcActor,
    target_user_id: Option<&str>,
    current_password: Option<&str>,
) -> Result<AppUserTotpSetupResult, String> {
    let actor_user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "permission_denied: authenticator setup requires user session".to_string()
        })?;
    let target_user_id = target_user_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(actor_user_id);
    if target_user_id != actor_user_id && !actor.is_admin() {
        return Err(
            "permission_denied: only admins may set up another user authenticator".to_string(),
        );
    }
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    let actor_user = storage
        .find_app_user_by_id(actor_user_id)
        .map_err(|err| format!("read actor user failed: {err}"))?
        .ok_or_else(|| "当前用户不存在".to_string())?;
    let setup_password_subject = throttle_subject("totp-setup-password", actor_user_id.as_bytes());
    reserve_auth_attempt_or_throttled(
        &mut storage,
        "totp_setup_password",
        &setup_password_subject,
        300,
        5,
        30,
        300,
    )?;
    let current_password = current_password.unwrap_or("");
    if current_password.as_bytes().len() > MAX_PASSWORD_BYTES
        || !verify_password_hash_guarded(current_password, &actor_user.password_hash)?
    {
        return Err("当前密码不正确".to_string());
    }
    storage
        .clear_auth_throttle("totp_setup_password", &setup_password_subject)
        .map_err(|err| format!("clear setup password throttle failed: {err}"))?;
    let user = storage
        .find_app_user_by_id(target_user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    begin_totp_setup_with_storage(&mut storage, &user, actor_user_id, SETUP_CHALLENGE_PURPOSE)
}

pub fn disable_app_user_totp(actor: &RpcActor) -> Result<(), String> {
    if actor.is_admin() {
        return Err("管理员不能关闭验证器".to_string());
    }
    let user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "permission_denied: authenticator disable requires user session".to_string()
        })?;
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    if !storage
        .disable_totp_and_revoke_user(user_id, now_ts())
        .map_err(|err| format!("disable authenticator and revoke sessions failed: {err}"))?
    {
        return Err("当前用户不存在".to_string());
    }
    Ok(())
}

pub fn app_user_totp_status(actor: &RpcActor) -> Result<AppUserTotpStatusResult, String> {
    let user_id = actor
        .user_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "permission_denied: authenticator status requires user session".to_string()
        })?;
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let state = storage
        .find_app_user_totp_state(user_id)
        .map_err(|err| format!("read authenticator state failed: {err}"))?
        .ok_or_else(|| "用户认证配置缺失".to_string())?;
    Ok(AppUserTotpStatusResult {
        enabled: state.enabled,
        confirmed_at: state.confirmed_at,
    })
}

pub fn reset_app_user_totp(actor: &RpcActor, user_id: &str) -> Result<(), String> {
    if !actor.is_admin() {
        return Err("permission_denied: only admins may reset authenticators".to_string());
    }
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Err("用户 ID 不能为空".to_string());
    }
    if actor.user_id.as_deref() == Some(user_id) {
        return Err("管理员不能重置自身验证器".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    storage
        .find_app_user_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    if !storage
        .disable_totp_and_revoke_user(user_id, now_ts())
        .map_err(|err| format!("reset authenticator and revoke sessions failed: {err}"))?
    {
        return Err("用户不存在".to_string());
    }
    Ok(())
}

fn begin_totp_setup_with_storage(
    storage: &mut Storage,
    user: &AppUser,
    initiated_by_user_id: &str,
    purpose: &str,
) -> Result<AppUserTotpSetupResult, String> {
    let secret = super::totp::generate_secret();
    let ciphertext = super::totp::encrypt_secret(&secret)?;
    let challenge_token = create_login_challenge(
        storage,
        &user.id,
        initiated_by_user_id,
        purpose,
        Some(ciphertext),
    )?;
    Ok(AppUserTotpSetupResult {
        user_id: user.id.clone(),
        username: user.username.clone(),
        otpauth_uri: super::totp::otpauth_uri(&user.username, &secret),
        secret,
        challenge_token: Some(challenge_token),
    })
}

fn create_login_challenge(
    storage: &mut Storage,
    user_id: &str,
    initiated_by_user_id: &str,
    purpose: &str,
    setup_secret_ciphertext: Option<String>,
) -> Result<String, String> {
    let token = format!("cmc_{}", random_hex(32));
    let now = now_ts();
    storage
        .replace_app_login_challenge(&AppLoginChallenge {
            id: generate_id("challenge", 8),
            user_id: user_id.to_string(),
            token_hash: token_hash(&token),
            purpose: purpose.to_string(),
            expires_at: now.saturating_add(300),
            attempts: 0,
            last_attempt_at: None,
            used_at: None,
            created_at: now,
            setup_secret_ciphertext,
            initiated_by_user_id: Some(initiated_by_user_id.to_string()),
            target_user_id: user_id.to_string(),
            locked_until: None,
        })
        .map_err(|err| format!("create login challenge failed: {err}"))?;
    Ok(token)
}

fn reserve_login_challenge(
    storage: &mut Storage,
    token: &str,
    purpose: &str,
) -> Result<AppLoginChallenge, String> {
    let challenge = storage
        .reserve_app_login_challenge_attempt(&token_hash(token), purpose, now_ts())
        .map_err(|err| format!("reserve login challenge attempt failed: {err}"))?;
    challenge.ok_or_else(|| "登录挑战已过期、锁定或尝试次数过多，请重新开始".to_string())
}

fn find_setup_challenge_purpose(storage: &Storage, token: &str) -> Result<String, String> {
    storage
        .find_active_app_login_challenge(&token_hash(token), now_ts())
        .map_err(|err| format!("read setup challenge failed: {err}"))?
        .map(|challenge| challenge.purpose)
        .ok_or_else(|| "绑定挑战已过期，请重新开始".to_string())
}

fn complete_app_login_challenge_with_storage(
    storage: &mut Storage,
    token: &str,
    totp_code: Option<&str>,
) -> Result<AppLoginAttempt, String> {
    let code = totp_code
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "请输入验证器验证码".to_string())?;
    let challenge = reserve_login_challenge(storage, token, LOGIN_CHALLENGE_PURPOSE)?;
    let totp_subject = throttle_subject("totp-login", challenge.target_user_id.as_bytes());
    let user = storage
        .find_app_user_by_id(&challenge.user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户名或密码错误".to_string())?;
    if user.status != "active" {
        return Err("登录挑战已失效，请重新登录".to_string());
    }
    let state = storage
        .find_app_user_totp_state(&user.id)
        .map_err(|err| format!("read authenticator state failed: {err}"))?
        .ok_or_else(|| "用户认证配置缺失".to_string())?;
    if !state.enabled {
        return Err("请先完成验证器绑定".to_string());
    }
    reserve_auth_attempt_or_throttled(storage, "totp_login", &totp_subject, 300, 5, 30, 300)?;
    let step = verify_user_totp(storage, &user.id, &state, code)?;
    let now = now_ts();
    let (session_token, session) = new_session(&user, now);
    if !storage
        .complete_app_totp_login(&challenge.id, &user.id, step, &session, now)
        .map_err(|err| format!("complete TOTP login failed: {err}"))?
    {
        return Err("验证码已使用或登录挑战已失效，请重新登录".to_string());
    }
    storage
        .clear_auth_throttle("totp_login", &totp_subject)
        .map_err(|err| format!("clear TOTP login throttle failed: {err}"))?;
    let mut next = user;
    next.last_login_at = Some(now);
    next.updated_at = now;
    Ok(AppLoginAttempt::Authenticated(session_result_with_storage(
        storage,
        next,
        session_token,
        &session,
    )?))
}

fn confirm_totp_setup_with_storage(
    storage: &mut Storage,
    challenge: &AppLoginChallenge,
    user: &AppUser,
    code: &str,
    session: Option<&AppUserSession>,
    current_session_id: Option<&str>,
    bootstrap: bool,
) -> Result<(), String> {
    let throttle_subject = throttle_subject("totp-setup", challenge.target_user_id.as_bytes());
    let secret_ciphertext = challenge
        .setup_secret_ciphertext
        .as_deref()
        .ok_or_else(|| "验证器绑定已失效，请重新开始".to_string())?;
    let secret = super::totp::decrypt_secret(secret_ciphertext)?;
    reserve_auth_attempt_or_throttled(storage, "totp_setup", &throttle_subject, 300, 5, 30, 300)?;
    let step = super::totp::verify_code(&secret, code, now_ts(), None)?;
    if !storage
        .complete_app_totp_setup(
            challenge,
            step,
            session,
            current_session_id,
            bootstrap,
            now_ts(),
        )
        .map_err(|err| format!("complete authenticator setup failed: {err}"))?
    {
        return Err("绑定挑战已失效，请重新开始".to_string());
    }
    storage
        .clear_auth_throttle("totp_setup", &throttle_subject)
        .map_err(|err| format!("clear TOTP setup throttle failed: {err}"))?;
    let _ = user;
    Ok(())
}

fn verify_user_totp(
    storage: &Storage,
    user_id: &str,
    state: &codexmanager_core::storage::AppUserTotpState,
    code: &str,
) -> Result<i64, String> {
    let ciphertext = state
        .secret_ciphertext
        .as_deref()
        .ok_or_else(|| "验证器配置缺失".to_string())?;
    let secret = super::totp::decrypt_secret(ciphertext)?;
    let step = super::totp::verify_code(&secret, code, now_ts(), state.last_used_step)?;
    let _ = user_id;
    let _ = storage;
    Ok(step)
}

fn app_session_user_result_from_storage(
    storage: &Storage,
    session: codexmanager_core::storage::AppSessionUserWithWallet,
    now: i64,
) -> Result<Option<AppSessionUserResult>, String> {
    if session.user.role == "admin" {
        let totp_enabled = storage
            .find_app_user_totp_state(&session.user.id)
            .map_err(|err| format!("read app user authenticator failed: {err}"))?
            .is_some_and(|state| state.enabled);
        if !totp_enabled {
            return Ok(None);
        }
    }
    let _ = storage.touch_app_user_session(&session.session_id, now);
    let mut user = public_user_with_wallet(session.user);
    fill_totp_state(&storage, &mut user)?;
    Ok(Some(AppSessionUserResult {
        session_id: session.session_id,
        expires_at: session.expires_at,
        user,
    }))
}

pub fn resolve_app_user_session(token: &str) -> Result<Option<AppSessionUserResult>, String> {
    let token = token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let now = now_ts();
    let token_hash = token_hash(token);
    let Some(session) = storage
        .find_active_app_session_user_by_token_hash(&token_hash, now)
        .map_err(|err| format!("read app session failed: {err}"))?
    else {
        return Ok(None);
    };
    app_session_user_result_from_storage(&storage, session, now)
}

pub(crate) fn resolve_app_user_session_by_id(
    session_id: &str,
) -> Result<Option<AppSessionUserResult>, String> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Ok(None);
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let now = now_ts();
    let Some(session) = storage
        .find_active_app_session_user_by_id(session_id, now)
        .map_err(|err| format!("read app session failed: {err}"))?
    else {
        return Ok(None);
    };
    app_session_user_result_from_storage(&storage, session, now)
}

pub fn logout_app_user_session(token: &str) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Ok(());
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    storage
        .revoke_app_user_session_by_token_hash(&token_hash(token), now_ts())
        .map_err(|err| format!("revoke app session failed: {err}"))?;
    Ok(())
}

pub fn create_app_user(input: AppUserCreateInput) -> Result<AppUserPublicResult, String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    create_app_user_with_storage(&storage, input)
}

pub fn list_app_users() -> Result<Vec<AppUserPublicResult>, String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    storage
        .list_public_app_users_with_wallets()
        .map_err(|err| format!("list app users failed: {err}"))?
        .into_iter()
        .map(|user| {
            let mut result = public_user_with_wallet(user);
            fill_totp_state(&storage, &mut result)?;
            Ok(result)
        })
        .collect()
}

pub fn update_app_user(input: AppUserUpdateInput) -> Result<AppUserPublicResult, String> {
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    let user_id = input.id.trim();
    if user_id.is_empty() {
        return Err("用户 ID 不能为空".to_string());
    }
    let current = storage
        .find_app_user_access_summary_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    let next_role = input
        .role
        .as_deref()
        .map(|value| normalize_role(Some(value)))
        .transpose()?
        .unwrap_or_else(|| current.role.clone());
    let next_status = input
        .status
        .as_deref()
        .map(normalize_status)
        .transpose()?
        .unwrap_or_else(|| current.status.clone());

    if current.role != "admin" && next_role == "admin" {
        let totp = storage
            .find_app_user_totp_state(user_id)
            .map_err(|err| format!("read authenticator state failed: {err}"))?
            .ok_or_else(|| "用户认证配置缺失".to_string())?;
        if !totp.enabled {
            return Err("晋升管理员前必须先绑定验证器".to_string());
        }
    }

    if current.role == "admin"
        && current.status == "active"
        && (next_role != "admin" || next_status != "active")
    {
        let active_admin_count = storage
            .active_admin_count()
            .map_err(|err| format!("read app admins failed: {err}"))?;
        if active_admin_count <= 1 {
            return Err("至少需要保留一个启用的管理员账号".to_string());
        }
    }

    storage
        .update_app_user_display_name(
            user_id,
            normalize_optional_text(input.display_name.as_deref()),
        )
        .map_err(|err| format!("update app user display name failed: {err}"))?;
    if (current.role != next_role || current.status != next_status)
        && !storage
            .update_role_status_and_revoke_user(user_id, &next_role, &next_status, now_ts())
            .map_err(|err| format!("update app user role/status failed: {err}"))?
    {
        return Err("角色或状态更新被拒绝；请确认管理员验证器和最后管理员约束".to_string());
    }
    let password_changed = normalize_optional_text(input.password.as_deref());
    if let Some(password) = password_changed.as_deref() {
        validate_password(password)?;
        let password_hash = hash_password_guarded(password)?;
        if !storage
            .update_password_and_revoke_other_sessions(user_id, &password_hash, None, now_ts())
            .map_err(|err| format!("update app user password failed: {err}"))?
        {
            return Err("用户不存在".to_string());
        }
    }
    let updated = storage
        .find_app_user_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    let wallet = if app_user_can_own_wallet(&updated) {
        Some(ensure_wallet(&storage, "user", &updated.id)?)
    } else {
        None
    };
    public_user_with_storage(&storage, updated, wallet)
}

pub fn delete_app_user(user_id: &str) -> Result<(), String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Err("用户 ID 不能为空".to_string());
    }
    let user = storage
        .find_app_user_access_summary_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    if user.role == "admin" && user.status == "active" {
        let active_admin_count = storage
            .active_admin_count()
            .map_err(|err| format!("read app admins failed: {err}"))?;
        if active_admin_count <= 1 {
            return Err("至少需要保留一个启用的管理员账号".to_string());
        }
    }
    let deleted = storage
        .delete_app_user(user_id)
        .map_err(|err| format!("delete app user failed: {err}"))?;
    if deleted == 0 {
        return Err("用户不存在".to_string());
    }
    Ok(())
}

pub fn list_api_key_owners() -> Result<Vec<ApiKeyOwnerResult>, String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    Ok(storage
        .list_api_key_owner_rows()
        .map_err(|err| format!("list api key owners failed: {err}"))?
        .into_iter()
        .map(api_key_owner_result)
        .collect())
}

pub fn list_api_key_ids_for_user(user_id: &str) -> Result<Vec<String>, String> {
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Ok(Vec::new());
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    storage
        .list_api_key_ids_for_user(user_id)
        .map_err(|err| format!("list api key ids for user failed: {err}"))
}

pub fn api_key_belongs_to_user(key_id: &str, user_id: &str) -> Result<bool, String> {
    let key_id = key_id.trim();
    let user_id = user_id.trim();
    if key_id.is_empty() || user_id.is_empty() {
        return Ok(false);
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let owner = storage
        .find_api_key_owner(key_id)
        .map_err(|err| format!("read api key owner failed: {err}"))?;
    Ok(owner.is_some_and(|owner| {
        owner.owner_kind == "user" && owner.owner_user_id.as_deref().map(str::trim) == Some(user_id)
    }))
}

pub fn update_app_user_profile(
    actor: &RpcActor,
    display_name: Option<&str>,
) -> Result<AppUserPublicResult, String> {
    let Some(user_id) = actor
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Err("permission_denied: profile requires user session".to_string());
    };
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    storage
        .update_app_user_display_name(user_id, normalize_optional_text(display_name))
        .map_err(|err| format!("update app user profile failed: {err}"))?;
    let user = storage
        .find_public_app_user_with_wallet_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "当前用户不存在".to_string())?;
    let mut result = public_user_with_wallet(user);
    fill_totp_state(&storage, &mut result)?;
    Ok(result)
}

pub fn change_app_user_password(
    actor: &RpcActor,
    current_password: &str,
    new_password: &str,
) -> Result<(), String> {
    let Some(user_id) = actor
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return Err("permission_denied: password change requires user session".to_string());
    };
    validate_password(new_password)?;
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    let user = storage
        .find_app_user_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "当前用户不存在".to_string())?;
    let password_subject = throttle_subject("password-change", user_id.as_bytes());
    reserve_auth_attempt_or_throttled(
        &mut storage,
        "password_change",
        &password_subject,
        300,
        5,
        30,
        300,
    )?;
    if current_password.as_bytes().len() > MAX_PASSWORD_BYTES
        || !verify_password_hash_guarded(current_password, &user.password_hash)?
    {
        return Err("当前密码不正确".to_string());
    }
    storage
        .clear_auth_throttle("password_change", &password_subject)
        .map_err(|err| format!("clear password change throttle failed: {err}"))?;
    let password_hash = hash_password_guarded(new_password)?;
    if !storage
        .update_password_and_revoke_other_sessions(
            user_id,
            &password_hash,
            actor.session_id.as_deref(),
            now_ts(),
        )
        .map_err(|err| format!("update password and revoke old sessions failed: {err}"))?
    {
        return Err("当前用户不存在".to_string());
    }
    Ok(())
}

pub fn wallet_top_up(
    owner_kind: &str,
    owner_id: &str,
    amount_credit_micros: i64,
    note: Option<&str>,
    created_by_user_id: Option<&str>,
) -> Result<AppWalletResult, String> {
    if amount_credit_micros <= 0 {
        return Err("充值金额必须大于 0".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let owner_kind = normalize_owner_kind(owner_kind)?;
    let owner_id = owner_id.trim();
    if owner_kind == "user" {
        let _ = ensure_user_can_own_wallet(&storage, owner_id)?;
    }
    let wallet = ensure_wallet(&storage, owner_kind, owner_id)?;
    let ledger = AppWalletLedgerEntry {
        id: generate_id("wl", 8),
        wallet_id: wallet.id.clone(),
        entry_kind: "manual_adjustment".to_string(),
        amount_credit_micros,
        balance_after_credit_micros: 0,
        request_log_id: None,
        api_key_id: None,
        pricing_rule_id: None,
        raw_usage_json: None,
        note: normalize_optional_text(note),
        created_by_user_id: normalize_optional_text(created_by_user_id),
        created_at: now_ts(),
    };
    let entry = storage
        .adjust_wallet_balance(&ledger)
        .map_err(|err| format!("adjust wallet failed: {err}"))?;
    let next = storage
        .find_wallet_by_owner(&wallet.owner_kind, &wallet.owner_id)
        .map_err(|err| format!("read app wallet failed: {err}"))?
        .ok_or_else(|| "钱包不存在".to_string())?;
    log::info!(
        "event=app_wallet_top_up wallet_id={} amount={} balance_after={}",
        entry.wallet_id,
        entry.amount_credit_micros,
        entry.balance_after_credit_micros
    );
    Ok(wallet_result(next))
}

pub fn wallet_set_available_credit(
    owner_kind: &str,
    owner_id: &str,
    available_credit_micros: i64,
    note: Option<&str>,
    created_by_user_id: Option<&str>,
) -> Result<AppWalletResult, String> {
    if available_credit_micros < 0 {
        return Err("可用额度必须是非负数字".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let owner_kind = normalize_owner_kind(owner_kind)?;
    let owner_id = owner_id.trim();
    if owner_kind == "user" {
        let _ = ensure_user_can_own_wallet(&storage, owner_id)?;
    }
    let wallet = ensure_wallet(&storage, owner_kind, owner_id)?;
    let target_balance = available_credit_micros.saturating_add(wallet.frozen_credit_micros);
    let delta = target_balance.saturating_sub(wallet.balance_credit_micros);
    if delta == 0 {
        return Ok(wallet_result(wallet));
    }
    let ledger = AppWalletLedgerEntry {
        id: generate_id("wl", 8),
        wallet_id: wallet.id.clone(),
        entry_kind: "manual_adjustment".to_string(),
        amount_credit_micros: delta,
        balance_after_credit_micros: 0,
        request_log_id: None,
        api_key_id: None,
        pricing_rule_id: None,
        raw_usage_json: None,
        note: normalize_optional_text(note).or_else(|| Some("set available credit".to_string())),
        created_by_user_id: normalize_optional_text(created_by_user_id),
        created_at: now_ts(),
    };
    let entry = storage
        .adjust_wallet_balance(&ledger)
        .map_err(|err| format!("set wallet credit failed: {err}"))?;
    let next = storage
        .find_wallet_by_owner(&wallet.owner_kind, &wallet.owner_id)
        .map_err(|err| format!("read app wallet failed: {err}"))?
        .ok_or_else(|| "钱包不存在".to_string())?;
    log::info!(
        "event=app_wallet_set_available wallet_id={} delta={} balance_after={}",
        entry.wallet_id,
        entry.amount_credit_micros,
        entry.balance_after_credit_micros
    );
    Ok(wallet_result(next))
}

pub fn set_api_key_owner(
    key_id: &str,
    owner_kind: &str,
    owner_user_id: Option<&str>,
    project_id: Option<&str>,
) -> Result<ApiKeyOwnerResult, String> {
    crate::initialize_storage_if_needed()?;
    let storage = open_storage_or_error()?;
    let key_id = key_id.trim();
    if key_id.is_empty() {
        return Err("API Key ID 不能为空".to_string());
    }
    if !storage
        .api_key_exists(key_id)
        .map_err(|err| format!("read api key failed: {err}"))?
    {
        return Err("API Key 不存在".to_string());
    }
    let owner_kind = normalize_owner_kind(owner_kind)?;
    let owner = match owner_kind {
        "user" => {
            let user_id = normalize_optional_text(owner_user_id)
                .ok_or_else(|| "用户归属需要 userId".to_string())?;
            let _ = ensure_user_can_own_wallet(&storage, &user_id)?;
            let _ = ensure_wallet(&storage, "user", &user_id)?;
            ApiKeyOwner {
                key_id: key_id.to_string(),
                owner_kind: owner_kind.to_string(),
                owner_user_id: Some(user_id),
                project_id: None,
                updated_at: now_ts(),
            }
        }
        "project" => {
            let project_id = normalize_optional_text(project_id)
                .ok_or_else(|| "项目归属需要 projectId".to_string())?;
            let _ = ensure_wallet(&storage, "project", &project_id)?;
            ApiKeyOwner {
                key_id: key_id.to_string(),
                owner_kind: owner_kind.to_string(),
                owner_user_id: None,
                project_id: Some(project_id),
                updated_at: now_ts(),
            }
        }
        _ => return Err("不支持的归属类型".to_string()),
    };
    storage
        .upsert_api_key_owner(&owner)
        .map_err(|err| format!("save api key owner failed: {err}"))?;
    Ok(api_key_owner_result(owner))
}

pub fn wallet_precheck_for_api_key(storage: &Storage, key_id: &str) -> Result<(), String> {
    wallet_precheck_for_api_key_rate(storage, key_id, None)
}

pub(crate) fn wallet_precheck_for_api_key_rate(
    storage: &Storage,
    key_id: &str,
    rate_multiplier_millis: Option<i64>,
) -> Result<(), String> {
    if !distribution_enabled_for_storage(storage) {
        return Ok(());
    }
    let Some(owner) = storage
        .find_api_key_owner(key_id)
        .map_err(|err| format!("read api key owner failed: {err}"))?
    else {
        return Ok(());
    };
    let (owner_kind, owner_id) = owner_identity(&owner)?;
    if owner_kind == "user" {
        let _ = ensure_user_can_own_wallet(storage, owner_id)?;
    }
    let wallet = storage
        .find_wallet_by_owner(owner_kind, owner_id)
        .map_err(|err| format!("read app wallet failed: {err}"))?
        .ok_or_else(|| "归属钱包不存在".to_string())?;
    if wallet.status != "active" {
        return Err("归属钱包已停用".to_string());
    }
    if rate_multiplier_millis.is_some_and(|value| value == 0) {
        return Ok(());
    }
    if wallet.balance_credit_micros <= wallet.frozen_credit_micros {
        return Err("归属钱包余额不足".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn record_request_charge_v2(
    storage: &Storage,
    key_id: Option<&str>,
    request_log_id: i64,
    model: &str,
    service_tier: Option<&str>,
    usage_source: &str,
    input_tokens: i64,
    cached_input_tokens: i64,
    cache_write_tokens: i64,
    output_tokens: i64,
    raw_usage_json: Option<String>,
    charge_wallet: bool,
) -> Result<codexmanager_core::storage::ChargeSnapshotV2, String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("model_slug_required".to_string());
    }
    let catalog_model = crate::models_v2::policy_catalog_slug(model);
    let now = now_ts();
    let mut wallet_id = None;
    let mut api_key_id = None;
    let mut billing_rule = None;
    let mut model_group_access = None;
    let mut multiplier_millis = 1_000;
    if charge_wallet && distribution_enabled_for_storage(storage) {
        if let Some(key_id) = key_id.map(str::trim).filter(|value| !value.is_empty()) {
            if let Some(owner) = storage
                .find_api_key_owner(key_id)
                .map_err(|err| format!("read api key owner failed: {err}"))?
            {
                let (owner_kind, owner_id) = owner_identity(&owner)?;
                if owner_kind == "user" {
                    let _ = ensure_user_can_own_wallet(storage, owner_id)?;
                }
                let wallet = storage
                    .find_wallet_by_owner(owner_kind, owner_id)
                    .map_err(|err| format!("read app wallet failed: {err}"))?
                    .ok_or_else(|| "归属钱包不存在".to_string())?;
                if wallet.status != "active" {
                    return Err("归属钱包已停用".to_string());
                }
                model_group_access =
                    crate::resolve_api_key_model_group_access(storage, key_id, model)?;
                billing_rule = if model_group_access.is_some() {
                    None
                } else {
                    resolve_billing_rule_for_request(
                        storage,
                        key_id,
                        &owner,
                        Some(model),
                        service_tier,
                        now,
                    )?
                };
                multiplier_millis = model_group_access
                    .as_ref()
                    .map(|access| access.rate_multiplier_millis.max(0))
                    .or_else(|| {
                        billing_rule
                            .as_ref()
                            .map(|rule| rule.multiplier_millis.max(0))
                    })
                    .unwrap_or(1_000);
                wallet_id = Some(wallet.id);
                api_key_id = Some(key_id.to_string());
            }
        }
    }
    let raw_usage_json = usage_json_with_billing_context(
        raw_usage_json,
        billing_rule.as_ref(),
        model_group_access.as_ref(),
        multiplier_millis,
    );
    storage
        .record_charge_snapshot_v2(&codexmanager_core::storage::ChargeSnapshotInputV2 {
            request_log_id,
            model_slug: model.to_string(),
            pricing_model_slug: (catalog_model != model).then(|| catalog_model.to_string()),
            usage_source: usage_source.to_string(),
            input_tokens,
            cached_input_tokens,
            cache_write_tokens,
            output_tokens,
            rate_multiplier_millis: multiplier_millis,
            wallet_id,
            api_key_id,
            pricing_rule_id: billing_rule.as_ref().map(|rule| rule.id.clone()),
            raw_usage_json,
            ledger_note: billing_rule
                .as_ref()
                .map(|rule| format!("billing_rule={}", rule.name))
                .or_else(|| {
                    model_group_access
                        .as_ref()
                        .map(|access| format!("model_group={}", access.group_id))
                }),
        })
        .map_err(|err| format!("record model catalog V2 charge failed: {err}"))
}

fn resolve_billing_rule_for_request(
    storage: &Storage,
    key_id: &str,
    owner: &ApiKeyOwner,
    model: Option<&str>,
    service_tier: Option<&str>,
    now: i64,
) -> Result<Option<BillingRule>, String> {
    let rules = storage
        .list_active_billing_rules_for_request_candidate(
            now,
            key_id,
            owner.owner_user_id.as_deref(),
            owner.project_id.as_deref(),
            service_tier,
            model,
        )
        .map_err(|err| format!("list billing rules failed: {err}"))?;
    Ok(rules
        .into_iter()
        .filter(|rule| billing_rule_matches(rule, key_id, owner, model, service_tier))
        .max_by_key(|rule| {
            (
                rule.priority,
                billing_rule_scope_score(rule),
                rule.model_pattern
                    .as_deref()
                    .map(str::len)
                    .unwrap_or_default() as i64,
                rule.updated_at,
            )
        }))
}

fn billing_rule_matches(
    rule: &BillingRule,
    key_id: &str,
    owner: &ApiKeyOwner,
    model: Option<&str>,
    service_tier: Option<&str>,
) -> bool {
    if !matches_optional_text(rule.api_key_id.as_deref(), Some(key_id)) {
        return false;
    }
    if !matches_optional_text(rule.user_id.as_deref(), owner.owner_user_id.as_deref()) {
        return false;
    }
    if !matches_optional_text(rule.project_id.as_deref(), owner.project_id.as_deref()) {
        return false;
    }
    if !matches_optional_text(rule.service_tier.as_deref(), service_tier) {
        return false;
    }
    billing_model_matches(rule.model_pattern.as_deref(), model)
}

fn matches_optional_text(rule_value: Option<&str>, context_value: Option<&str>) -> bool {
    let Some(rule_value) = rule_value.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    context_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| value.eq_ignore_ascii_case(rule_value))
}

fn billing_model_matches(rule_pattern: Option<&str>, model: Option<&str>) -> bool {
    let Some(pattern) = rule_pattern
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "*")
    else {
        return true;
    };
    let Some(model) = model
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("unknown"))
    else {
        return false;
    };
    let pattern = pattern.to_ascii_lowercase();
    let model = model.to_ascii_lowercase();
    if pattern.contains('*') {
        crate::quota::model_pricing::wildcard_matches(&pattern, &model)
    } else {
        model.starts_with(&pattern)
    }
}

fn billing_rule_scope_score(rule: &BillingRule) -> i64 {
    [
        rule.api_key_id.as_deref(),
        rule.user_id.as_deref(),
        rule.project_id.as_deref(),
        rule.service_tier.as_deref(),
        rule.model_pattern.as_deref(),
    ]
    .into_iter()
    .filter(|value| value.map(str::trim).is_some_and(|text| !text.is_empty()))
    .count() as i64
}

fn usage_json_with_billing_context(
    raw_usage_json: Option<String>,
    billing_rule: Option<&BillingRule>,
    model_group_access: Option<&codexmanager_core::storage::ModelGroupAccess>,
    multiplier_millis: i64,
) -> Option<String> {
    let mut value = raw_usage_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !value.is_object() {
        value = serde_json::json!({ "raw": value });
    }
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "billingMultiplierMillis".to_string(),
            serde_json::json!(multiplier_millis.max(0)),
        );
        if let Some(rule) = billing_rule {
            object.insert("billingRuleId".to_string(), serde_json::json!(rule.id));
            object.insert("billingRuleName".to_string(), serde_json::json!(rule.name));
        }
        if let Some(access) = model_group_access {
            object.insert(
                "modelGroupId".to_string(),
                serde_json::json!(access.group_id),
            );
            object.insert(
                "modelGroupName".to_string(),
                serde_json::json!(access.group_name),
            );
            object.insert(
                "platformModelSlug".to_string(),
                serde_json::json!(access.platform_model_slug),
            );
        }
    }
    serde_json::to_string(&value).ok()
}

fn open_storage_or_error() -> Result<crate::storage_helpers::StorageHandle, String> {
    open_storage().ok_or_else(|| "存储不可用".to_string())
}

fn create_app_user_with_storage(
    storage: &Storage,
    input: AppUserCreateInput,
) -> Result<AppUserPublicResult, String> {
    let username = normalize_username(&input.username)?;
    validate_password(&input.password)?;
    if storage
        .app_username_exists(&username)
        .map_err(|err| format!("read app user failed: {err}"))?
    {
        return Err("用户名已存在".to_string());
    }
    let role = normalize_role(input.role.as_deref())?;
    if role == "admin" && input.initial_balance_credit_micros.unwrap_or(0) > 0 {
        return Err("管理员账号不参与额度分发".to_string());
    }
    let now = now_ts();
    let user = AppUser {
        id: generate_id("usr", 8),
        username,
        display_name: normalize_optional_text(input.display_name.as_deref()),
        password_hash: hash_password_guarded(&input.password)?,
        role,
        status: "active".to_string(),
        created_at: now,
        updated_at: now,
        last_login_at: None,
    };
    storage
        .insert_app_user(&user)
        .map_err(|err| format!("create app user failed: {err}"))?;
    let wallet = if app_user_can_own_wallet(&user) {
        storage
            .assign_default_model_group_to_user(&user.id)
            .map_err(|err| format!("assign default model group failed: {err}"))?;
        Some(ensure_wallet(storage, "user", &user.id)?)
    } else {
        None
    };
    if let Some(initial_balance) = input
        .initial_balance_credit_micros
        .filter(|value| *value > 0)
    {
        let wallet = wallet
            .as_ref()
            .ok_or_else(|| "管理员账号不参与额度分发".to_string())?;
        let ledger = AppWalletLedgerEntry {
            id: generate_id("wl", 8),
            wallet_id: wallet.id.clone(),
            entry_kind: "initial_grant".to_string(),
            amount_credit_micros: initial_balance,
            balance_after_credit_micros: 0,
            request_log_id: None,
            api_key_id: None,
            pricing_rule_id: None,
            raw_usage_json: None,
            note: Some("initial balance".to_string()),
            created_by_user_id: None,
            created_at: now_ts(),
        };
        let _ = storage
            .adjust_wallet_balance(&ledger)
            .map_err(|err| format!("grant app wallet failed: {err}"))?;
    }
    let wallet = if app_user_can_own_wallet(&user) {
        storage
            .find_wallet_by_owner("user", &user.id)
            .map_err(|err| format!("read app wallet failed: {err}"))?
    } else {
        None
    };
    public_user_with_storage(storage, user, wallet)
}

pub fn create_app_user_for_actor(
    actor: &RpcActor,
    input: AppUserCreateInput,
    actor_password: &str,
) -> Result<AppUserCreateResult, String> {
    if !actor.is_admin() {
        return Err("permission_denied: only admins may create users".to_string());
    }
    crate::initialize_storage_if_needed()?;
    let mut storage = open_storage_or_error()?;
    let role = normalize_role(input.role.as_deref())?;
    if role == "admin" {
        super::totp::validate_encryption_key()?;
        if let Some(actor_user_id) = actor.user_id.as_deref() {
            let actor_user = storage
                .find_app_user_by_id(actor_user_id)
                .map_err(|err| format!("read actor user failed: {err}"))?
                .ok_or_else(|| "当前管理员不存在".to_string())?;
            let password_subject =
                throttle_subject("admin-create-password", actor_user_id.as_bytes());
            reserve_auth_attempt_or_throttled(
                &mut storage,
                "admin_create_password",
                &password_subject,
                300,
                5,
                30,
                300,
            )?;
            if actor_password.as_bytes().len() > MAX_PASSWORD_BYTES
                || !verify_password_hash_guarded(actor_password, &actor_user.password_hash)?
            {
                return Err("当前管理员密码不正确".to_string());
            }
            storage
                .clear_auth_throttle("admin_create_password", &password_subject)
                .map_err(|err| format!("clear administrator password throttle failed: {err}"))?;
        }
    }
    let user = create_app_user_with_storage(&storage, input)?;
    let totp_setup = if role == "admin" {
        let target = storage
            .find_app_user_by_id(&user.id)
            .map_err(|err| format!("read new administrator failed: {err}"))?
            .ok_or_else(|| "新管理员创建失败".to_string())?;
        Some(begin_totp_setup_with_storage(
            &mut storage,
            &target,
            actor.user_id.as_deref().unwrap_or(&target.id),
            SETUP_CHALLENGE_PURPOSE,
        )?)
    } else {
        None
    };
    Ok(AppUserCreateResult { user, totp_setup })
}

fn create_session_with_storage(storage: &Storage, user: AppUser) -> Result<AppLoginResult, String> {
    let now = now_ts();
    let (token, session) = new_session(&user, now);
    storage
        .insert_app_user_session(&session)
        .map_err(|err| format!("create app session failed: {err}"))?;
    session_result_with_storage(storage, user, token, &session)
}

fn new_session(user: &AppUser, now: i64) -> (String, AppUserSession) {
    let token = generate_session_token();
    let session = AppUserSession {
        id: generate_id("sess", 8),
        user_id: user.id.clone(),
        token_hash: token_hash(&token),
        expires_at: now.saturating_add(SESSION_TTL_SECONDS),
        created_at: now,
        last_seen_at: Some(now),
        revoked_at: None,
    };
    (token, session)
}

fn session_result_with_storage(
    storage: &Storage,
    user: AppUser,
    token: String,
    session: &AppUserSession,
) -> Result<AppLoginResult, String> {
    let wallet = if app_user_can_own_wallet(&user) {
        storage
            .find_wallet_by_owner("user", &user.id)
            .map_err(|err| format!("read app wallet failed: {err}"))?
    } else {
        None
    };
    let public_user = public_user_with_storage(storage, user, wallet)?;
    Ok(AppLoginResult {
        token,
        expires_at: session.expires_at,
        user: public_user,
    })
}

fn ensure_wallet(storage: &Storage, owner_kind: &str, owner_id: &str) -> Result<AppWallet, String> {
    let owner_kind = normalize_owner_kind(owner_kind)?;
    let owner_id = owner_id.trim();
    if owner_id.is_empty() {
        return Err("钱包归属 ID 不能为空".to_string());
    }
    storage
        .ensure_wallet_for_owner(&generate_id("wlt", 8), owner_kind, owner_id)
        .map_err(|err| format!("ensure app wallet failed: {err}"))
}

fn app_user_can_own_wallet(user: &AppUser) -> bool {
    user.role != "admin"
}

fn ensure_user_can_own_wallet(
    storage: &Storage,
    user_id: &str,
) -> Result<AppUserAccessSummary, String> {
    let user_id = user_id.trim();
    if user_id.is_empty() {
        return Err("用户归属需要 userId".to_string());
    }
    let user = storage
        .find_app_user_access_summary_by_id(user_id)
        .map_err(|err| format!("read app user failed: {err}"))?
        .ok_or_else(|| "用户不存在".to_string())?;
    if user.role == "admin" {
        return Err("管理员账号不参与额度分发".to_string());
    }
    if user.status != "active" {
        return Err("用户已禁用".to_string());
    }
    Ok(user)
}

fn public_user(user: AppUser, wallet: Option<AppWallet>) -> AppUserPublicResult {
    let can_own_wallet = app_user_can_own_wallet(&user);
    AppUserPublicResult {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
        updated_at: user.updated_at,
        last_login_at: user.last_login_at,
        totp_enabled: false,
        totp_confirmed_at: None,
        wallet: if can_own_wallet {
            wallet.map(wallet_result)
        } else {
            None
        },
    }
}

fn public_user_with_wallet(user: PublicAppUserWithWallet) -> AppUserPublicResult {
    let wallet = public_wallet_result(&user);
    AppUserPublicResult {
        id: user.id,
        username: user.username,
        display_name: user.display_name,
        role: user.role,
        status: user.status,
        created_at: user.created_at,
        updated_at: user.updated_at,
        last_login_at: user.last_login_at,
        totp_enabled: false,
        totp_confirmed_at: None,
        wallet,
    }
}

fn public_wallet_result(user: &PublicAppUserWithWallet) -> Option<AppWalletResult> {
    let id = user.wallet_id.clone()?;
    let balance_credit_micros = user.wallet_balance_credit_micros?;
    let frozen_credit_micros = user.wallet_frozen_credit_micros?;
    Some(AppWalletResult {
        id,
        owner_kind: user.wallet_owner_kind.clone()?,
        owner_id: user.wallet_owner_id.clone()?,
        balance_credit_micros,
        frozen_credit_micros,
        available_credit_micros: (balance_credit_micros - frozen_credit_micros).max(0),
        status: user.wallet_status.clone()?,
        created_at: user.wallet_created_at?,
        updated_at: user.wallet_updated_at?,
    })
}

fn wallet_result(wallet: AppWallet) -> AppWalletResult {
    AppWalletResult {
        available_credit_micros: (wallet.balance_credit_micros - wallet.frozen_credit_micros)
            .max(0),
        id: wallet.id,
        owner_kind: wallet.owner_kind,
        owner_id: wallet.owner_id,
        balance_credit_micros: wallet.balance_credit_micros,
        frozen_credit_micros: wallet.frozen_credit_micros,
        status: wallet.status,
        created_at: wallet.created_at,
        updated_at: wallet.updated_at,
    }
}

fn api_key_owner_result(owner: ApiKeyOwner) -> ApiKeyOwnerResult {
    ApiKeyOwnerResult {
        key_id: owner.key_id,
        owner_kind: owner.owner_kind,
        owner_user_id: owner.owner_user_id,
        project_id: owner.project_id,
        updated_at: owner.updated_at,
    }
}

fn owner_identity(owner: &ApiKeyOwner) -> Result<(&str, &str), String> {
    match owner.owner_kind.as_str() {
        "user" => owner
            .owner_user_id
            .as_deref()
            .map(|id| ("user", id))
            .ok_or_else(|| "API Key 用户归属缺失".to_string()),
        "project" => owner
            .project_id
            .as_deref()
            .map(|id| ("project", id))
            .ok_or_else(|| "API Key 项目归属缺失".to_string()),
        _ => Err("API Key 归属类型无效".to_string()),
    }
}

fn normalize_web_auth_mode(raw: Option<&str>) -> &'static str {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some(WEB_AUTH_MODE_PASSWORD) => WEB_AUTH_MODE_PASSWORD,
        Some(WEB_AUTH_MODE_ACCOUNTS) => WEB_AUTH_MODE_ACCOUNTS,
        _ => WEB_AUTH_MODE_NONE,
    }
}

fn normalize_owner_kind(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "user" => Ok("user"),
        "project" => Ok("project"),
        _ => Err("归属类型必须是 user 或 project".to_string()),
    }
}

fn normalize_role(raw: Option<&str>) -> Result<String, String> {
    let role = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("member")
        .to_ascii_lowercase();
    match role.as_str() {
        "admin" | "member" => Ok(role),
        _ => Err("角色必须是 admin 或 member".to_string()),
    }
}

fn normalize_status(raw: &str) -> Result<String, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "active" => Ok("active".to_string()),
        "disabled" => Ok("disabled".to_string()),
        _ => Err("状态必须是 active 或 disabled".to_string()),
    }
}

fn normalize_username(raw: &str) -> Result<String, String> {
    let value = raw.trim().to_ascii_lowercase();
    if value.len() < 3 || value.len() > 64 {
        return Err("用户名长度需要在 3 到 64 之间".to_string());
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err("用户名仅支持字母、数字、点、下划线和短横线".to_string());
    }
    Ok(value)
}

fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("密码至少需要 8 位".to_string());
    }
    if password.as_bytes().len() > MAX_PASSWORD_BYTES {
        return Err("密码不能超过 256 字节".to_string());
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut PasswordOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| "密码哈希失败".to_string())
}

struct ArgonPermit;

fn argon_gate() -> &'static (Mutex<usize>, Condvar) {
    static GATE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();
    GATE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

fn acquire_argon_permit() -> Result<ArgonPermit, String> {
    let (mutex, available) = argon_gate();
    let active = mutex.lock().map_err(|_| AUTH_THROTTLED_ERROR.to_string())?;
    let (mut active, timeout) = available
        .wait_timeout_while(active, Duration::from_secs(3), |count| *count >= 2)
        .map_err(|_| AUTH_THROTTLED_ERROR.to_string())?;
    if timeout.timed_out() && *active >= 2 {
        return Err(AUTH_THROTTLED_ERROR.to_string());
    }
    *active += 1;
    Ok(ArgonPermit)
}

impl Drop for ArgonPermit {
    fn drop(&mut self) {
        let (mutex, available) = argon_gate();
        if let Ok(mut active) = mutex.lock() {
            *active = active.saturating_sub(1);
            available.notify_one();
        }
    }
}

fn hash_password_guarded(password: &str) -> Result<String, String> {
    let _permit = acquire_argon_permit()?;
    hash_password(password)
}

fn verify_password_hash_guarded(password: &str, stored_hash: &str) -> Result<bool, String> {
    let _permit = acquire_argon_permit()?;
    if !stored_hash.starts_with("$argon2") {
        let hash = dummy_argon_hash();
        let _ = verify_password_hash("invalid-login-password", hash);
    }
    Ok(verify_password_hash(password, stored_hash))
}

fn verify_dummy_password_guarded() -> Result<(), String> {
    let _permit = acquire_argon_permit()?;
    let _ = verify_password_hash("invalid-login-password", dummy_argon_hash());
    Ok(())
}

fn dummy_argon_hash() -> &'static str {
    static DUMMY_HASH: OnceLock<String> = OnceLock::new();
    DUMMY_HASH
        .get_or_init(|| {
            hash_password("CodexManager-dummy-password-never-valid")
                .expect("dummy Argon2id hash must be constructible")
        })
        .as_str()
}

fn throttle_subject(scope: &str, value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"codexmanager-auth-throttle:");
    hasher.update(scope.as_bytes());
    hasher.update(b":");
    hasher.update(value);
    hex_encode(hasher.finalize().as_slice())
}

fn reserve_auth_attempt_or_throttled(
    storage: &mut Storage,
    scope: &str,
    subject_hash: &str,
    window_seconds: i64,
    threshold: i64,
    base_lock_seconds: i64,
    max_lock_seconds: i64,
) -> Result<(), String> {
    let reserved = storage
        .reserve_auth_attempt(
            scope,
            subject_hash,
            now_ts(),
            window_seconds,
            threshold,
            base_lock_seconds,
            max_lock_seconds,
        )
        .map_err(|err| format!("reserve authentication attempt failed: {err}"))?;
    if !reserved {
        return Err(AUTH_THROTTLED_ERROR.to_string());
    }
    Ok(())
}

fn public_user_with_storage(
    storage: &Storage,
    user: AppUser,
    wallet: Option<AppWallet>,
) -> Result<AppUserPublicResult, String> {
    let mut result = public_user(user, wallet);
    fill_totp_state(storage, &mut result)?;
    Ok(result)
}

fn public_user_with_totp_state(
    user: AppUser,
    wallet: Option<AppWallet>,
    state: &codexmanager_core::storage::AppUserTotpState,
) -> AppUserPublicResult {
    let mut result = public_user(user, wallet);
    result.totp_enabled = state.enabled;
    result.totp_confirmed_at = state.confirmed_at;
    result
}

fn fill_totp_state(storage: &Storage, result: &mut AppUserPublicResult) -> Result<(), String> {
    if let Some(state) = storage
        .find_app_user_totp_state(&result.id)
        .map_err(|err| format!("read authenticator state failed: {err}"))?
    {
        result.totp_enabled = state.enabled;
        result.totp_confirmed_at = state.confirmed_at;
    }
    Ok(())
}

fn verify_password_hash(password: &str, stored_hash: &str) -> bool {
    if stored_hash.starts_with("$argon2") {
        return PasswordHash::new(stored_hash).ok().is_some_and(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        });
    }
    let mut parts = stored_hash.split('$');
    let Some(kind) = parts.next() else {
        return false;
    };
    let Some(salt_hex) = parts.next() else {
        return false;
    };
    let Some(expected_hash) = parts.next() else {
        return false;
    };
    if kind != "sha256" || parts.next().is_some() {
        return false;
    }
    super::rpc::constant_time_eq(
        hex_sha256(format!("{salt_hex}:{password}").as_bytes()).as_bytes(),
        expected_hash.as_bytes(),
    )
}

fn token_hash(token: &str) -> String {
    hex_sha256(format!("codexmanager-app-session:{token}").as_bytes())
}

fn generate_session_token() -> String {
    format!("cms_{}", random_hex(32))
}

fn generate_id(prefix: &str, bytes_len: usize) -> String {
    format!("{prefix}_{}", random_hex(bytes_len))
}

fn random_hex(bytes_len: usize) -> String {
    let mut bytes = vec![0u8; bytes_len];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

fn hex_sha256(bytes: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_ref());
    let digest = hasher.finalize();
    hex_encode(digest.as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
