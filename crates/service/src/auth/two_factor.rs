//! Optional second factor for the existing administrator login. No auth-mode migration.
use super::{two_factor_crypto as crypto, two_factor_storage as storage};
use codexmanager_core::storage::now_ts;
use serde::{Deserialize, Serialize};

const INVALID: &str = "动态验证码或恢复码错误、已使用或已过期";
const LIMITED: &str = "验证尝试过于频繁，请五分钟后重试";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    secret: Option<String>,
    generation: String,
    last_step: Option<i64>,
    recovery: Vec<String>,
    pending: Option<Pending>,
    tickets: Vec<Ticket>,
    failures: Vec<i64>,
    management_attempts: Vec<i64>,
}
#[derive(Serialize, Deserialize)]
struct Pending {
    secret: String,
    token_hash: String,
    actor: String,
    expires: i64,
    attempts: u8,
}
#[derive(Serialize, Deserialize)]
struct Ticket {
    token_hash: String,
    payload: String,
    expires: i64,
    attempts: u8,
    generation: String,
}
#[derive(Serialize, Deserialize)]
struct LoginPayload {
    mode: String,
    token: String,
    fingerprint: String,
    user_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoFactorStatus {
    pub enabled: bool,
    pub recovery_codes_remaining: usize,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoFactorSetup {
    pub secret: String,
    pub otpauth_uri: String,
    pub setup_token: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TwoFactorConfirmation {
    pub recovery_codes: Vec<String>,
}
pub struct TwoFactorLogin {
    pub token: String,
    pub receipt: String,
}

fn read() -> Result<State, String> {
    crate::initialize_storage_if_needed()?;
    serde_json::from_str(&storage::load()?.state_json)
        .map_err(|_| "2FA 状态损坏，请检查数据库备份".into())
}

// Persist failed attempts as well as successful operations. CAS prevents parallel
// requests from accepting a code/recovery code twice or resurrecting revoked state.
fn update<T>(mut action: impl FnMut(&mut State) -> Result<T, String>) -> Result<T, String> {
    crate::initialize_storage_if_needed()?;
    for _ in 0..12 {
        let snapshot = storage::load()?;
        let mut state: State =
            serde_json::from_str(&snapshot.state_json).map_err(|_| "2FA 状态损坏")?;
        let now = now_ts();
        state.tickets.retain(|t| t.expires > now && t.attempts < 5);
        state.failures.retain(|ts| *ts > now - 300);
        state.management_attempts.retain(|ts| *ts > now - 300);
        let result = action(&mut state);
        if storage::compare_exchange(
            snapshot.version,
            serde_json::to_string(&state).map_err(|e| e.to_string())?,
        )? {
            return result;
        }
    }
    Err("2FA 状态正在更新，请重试".into())
}

pub fn status() -> Result<TwoFactorStatus, String> {
    let state = read()?;
    Ok(TwoFactorStatus {
        enabled: state.secret.is_some(),
        recovery_codes_remaining: state.recovery.len(),
    })
}

fn verify_password(actor: &crate::RpcActor, password: &str) -> Result<(), String> {
    if !actor.is_admin() {
        return Err("permission_denied: 2FA".into());
    }
    update(|s| {
        if s.management_attempts.len() >= 10 {
            return Err(LIMITED.into());
        }
        s.management_attempts.push(now_ts());
        Ok(())
    })?;
    let hash = storage::password_hash(actor)?;
    if !super::app_manager::verify_password_hash(password, &hash) {
        return Err("当前密码错误".into());
    }
    Ok(())
}

pub fn begin_setup(actor: &crate::RpcActor, password: &str) -> Result<TwoFactorSetup, String> {
    verify_password(actor, password)?;
    let secret = crypto::generate_secret();
    let encrypted = crypto::seal(&secret)?;
    let setup_token = crypto::random_hex(32);
    update(|s| {
        if s.secret.is_some() {
            return Err("2FA 已启用，请先验证并关闭后再更换验证器".into());
        }
        s.pending = Some(Pending {
            secret: encrypted.clone(),
            token_hash: crypto::hash(&setup_token),
            actor: actor.user_id.clone().unwrap_or_default(),
            expires: now_ts() + 600,
            attempts: 0,
        });
        Ok(())
    })?;
    Ok(TwoFactorSetup { otpauth_uri: format!("otpauth://totp/CodexManager:Admin?secret={secret}&issuer=CodexManager&algorithm=SHA1&digits=6&period=30"), secret, setup_token })
}

pub fn confirm_setup(
    actor: &crate::RpcActor,
    password: &str,
    setup_token: &str,
    code: &str,
) -> Result<TwoFactorConfirmation, String> {
    verify_password(actor, password)?;
    let recovery_codes: Vec<String> = (0..8).map(|_| crypto::random_hex(12)).collect();
    update(|s| {
        if s.secret.is_some() {
            return Err("2FA 已启用".into());
        }
        let pending = s
            .pending
            .as_mut()
            .filter(|p| {
                p.expires > now_ts()
                    && p.attempts < 5
                    && p.token_hash == crypto::hash(setup_token)
                    && p.actor == actor.user_id.clone().unwrap_or_default()
            })
            .ok_or("绑定已失效，请重新开始")?;
        pending.attempts += 1;
        let step = crypto::verify(&crypto::unseal(&pending.secret)?, code, now_ts(), None)?;
        s.secret = Some(pending.secret.clone());
        s.pending = None;
        s.generation = crypto::random_hex(32);
        s.last_step = Some(step);
        s.recovery = recovery_codes.iter().map(|c| crypto::hash(c)).collect();
        s.tickets.clear();
        Ok(TwoFactorConfirmation {
            recovery_codes: recovery_codes.clone(),
        })
    })
}

fn consume_code(s: &mut State, value: &str) -> Result<(), String> {
    if s.failures.len() >= 20 {
        return Err(LIMITED.into());
    }
    s.failures.push(now_ts());
    let secret = crypto::unseal(s.secret.as_deref().ok_or("2FA 未启用")?)?;
    if let Ok(step) = crypto::verify(&secret, value, now_ts(), s.last_step) {
        s.last_step = Some(step);
        s.failures.pop();
        return Ok(());
    }
    let hash = crypto::hash(&value.trim().to_ascii_lowercase());
    if let Some(index) = s
        .recovery
        .iter()
        .position(|candidate| super::rpc::constant_time_eq(candidate.as_bytes(), hash.as_bytes()))
    {
        s.recovery.remove(index);
        s.failures.pop();
        return Ok(());
    }
    Err(INVALID.into())
}

pub fn disable(actor: &crate::RpcActor, password: &str, code: &str) -> Result<(), String> {
    verify_password(actor, password)?;
    update(|s| {
        consume_code(s, code)?;
        s.secret = None;
        s.pending = None;
        s.recovery.clear();
        s.tickets.clear();
        s.last_step = None;
        s.generation = crypto::random_hex(32);
        Ok(())
    })
}

pub fn issue_login(
    mode: &str,
    token: &str,
    user_id: Option<String>,
) -> Result<Option<String>, String> {
    let actor = crate::RpcActor::from_parts(Some("admin"), user_id.as_deref());
    let payload = LoginPayload {
        mode: mode.into(),
        token: token.into(),
        fingerprint: crypto::hash(&storage::password_hash(&actor)?),
        user_id,
    };
    let token = crypto::random_hex(32);
    let encrypted = crypto::seal(&serde_json::to_string(&payload).map_err(|e| e.to_string())?)?;
    update(|s| {
        if s.secret.is_none() {
            return Ok(None);
        }
        if s.tickets.len() >= 32 {
            return Err(LIMITED.into());
        }
        s.tickets.push(Ticket {
            token_hash: crypto::hash(&token),
            payload: encrypted.clone(),
            expires: now_ts() + 300,
            attempts: 0,
            generation: s.generation.clone(),
        });
        Ok(Some(token.clone()))
    })
}

pub fn complete_login(challenge: &str, value: &str) -> Result<TwoFactorLogin, String> {
    update(|s| {
        let ticket = s
            .tickets
            .iter_mut()
            .find(|t| t.token_hash == crypto::hash(challenge) && t.generation == s.generation)
            .ok_or("登录验证已失效，请重新输入密码")?;
        ticket.attempts += 1;
        let payload: LoginPayload = serde_json::from_str(&crypto::unseal(&ticket.payload)?)
            .map_err(|_| "登录验证数据无效")?;
        if payload.mode != crate::current_web_auth_mode() {
            return Err("登录方式已变更，请重新登录".into());
        }
        let actor = crate::RpcActor::from_parts(Some("admin"), payload.user_id.as_deref());
        if crypto::hash(&storage::password_hash(&actor)?) != payload.fingerprint {
            return Err("密码已变更，请重新登录".into());
        }
        if payload.mode == "accounts" && crate::resolve_app_user_session(&payload.token)?.is_none()
        {
            return Err("登录会话已失效".into());
        }
        consume_code(s, value)?;
        s.tickets
            .retain(|t| t.token_hash != crypto::hash(challenge));
        Ok(TwoFactorLogin {
            receipt: crypto::receipt(&payload.mode, &payload.token, &s.generation),
            token: payload.token,
        })
    })
}

pub fn receipt_valid(mode: &str, token: &str, receipt: &str) -> Result<bool, String> {
    let s = read()?;
    Ok(s.secret.is_none()
        || super::rpc::constant_time_eq(
            crypto::receipt(mode, token, &s.generation).as_bytes(),
            receipt.as_bytes(),
        ))
}

/// Used only by the authenticated Web RPC response after successful enrollment.
pub fn current_receipt(mode: &str, token: &str) -> Result<String, String> {
    let state = read()?;
    if state.secret.is_none() {
        return Err("2FA 未启用".into());
    }
    Ok(crypto::receipt(mode, token, &state.generation))
}

#[cfg(test)]
#[path = "tests/two_factor_tests.rs"]
mod tests;
