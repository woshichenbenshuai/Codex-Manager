use codexmanager_core::rpc::types::{JsonRpcRequest, JsonRpcResponse};

use crate::RpcActor;

/// 函数 `try_handle`
///
/// 作者: gaohongshun
///
/// 时间: 2026-05-11
///
/// # 参数
/// - req: 参数 req
///
/// # 返回
/// 返回函数执行结果
pub(super) fn try_handle(req: &JsonRpcRequest, actor: &RpcActor) -> Option<JsonRpcResponse> {
    if matches!(
        req.method.as_str(),
        "accountManager/status"
            | "accountManager/users/list"
            | "accountManager/users/create"
            | "accountManager/users/update"
            | "accountManager/users/delete"
            | "accountManager/users/totp/reset"
            | "accountManager/wallet/topUp"
            | "accountManager/wallet/setAvailable"
            | "accountManager/apiKeyOwners/list"
            | "accountManager/apiKeyOwners/set"
            | "accountManager/webAuthMode/set"
            | "accountManager/distribution/set"
    ) && !actor.is_admin()
    {
        return Some(super::response(
            req,
            super::value_or_error::<()>(Err(format!("permission_denied: {}", req.method))),
        ));
    }
    let result = match req.method.as_str() {
        "accountManager/status" => super::value_or_error(crate::app_auth_status_value()),
        "accountManager/session/current" => super::value_or_error(crate::app_session_result(actor)),
        "accountManager/session/list" => {
            super::value_or_error(crate::list_app_user_sessions(actor))
        }
        "accountManager/session/revoke" => {
            let session_id = super::str_param(req, "sessionId").unwrap_or("");
            super::ok_or_error(crate::revoke_app_user_session(actor, session_id))
        }
        "accountManager/profile/update" => {
            let display_name = super::str_param(req, "displayName");
            super::value_or_error(crate::update_app_user_profile(actor, display_name))
        }
        "accountManager/password/change" => {
            let current_password = super::str_param(req, "currentPassword").unwrap_or("");
            let new_password = super::str_param(req, "newPassword").unwrap_or("");
            super::ok_or_error(crate::change_app_user_password(
                actor,
                current_password,
                new_password,
            ))
        }
        "accountManager/totp/status" => super::value_or_error(crate::app_user_totp_status(actor)),
        "accountManager/totp/setup/begin" => {
            let target_user_id = super::str_param(req, "userId");
            let current_password = super::str_param(req, "currentPassword");
            super::value_or_error(crate::begin_app_user_totp_setup(
                actor,
                target_user_id,
                current_password,
            ))
        }
        "accountManager/totp/setup/confirm" => {
            let code = super::str_param(req, "code").unwrap_or("");
            let challenge_token = super::str_param(req, "challengeToken").unwrap_or("");
            let target_user_id = super::str_param(req, "targetUserId").unwrap_or("");
            super::value_or_error(crate::confirm_app_user_totp_setup_for_actor(
                actor,
                target_user_id,
                challenge_token,
                code,
            ))
        }
        "accountManager/totp/disable" => super::ok_or_error(crate::disable_app_user_totp(actor)),
        "accountManager/users/list" => super::value_or_error(crate::list_app_users()),
        "accountManager/users/create" => {
            let actor_password = super::str_param(req, "actorPassword")
                .unwrap_or("")
                .to_string();
            let input = req
                .params
                .clone()
                .map(serde_json::from_value::<crate::AppUserCreateInput>)
                .transpose()
                .map_err(|err| format!("invalid user payload: {err}"));
            super::value_or_error(
                input
                    .and_then(|input| input.ok_or_else(|| "missing user payload".to_string()))
                    .and_then(|input| {
                        crate::create_app_user_for_actor(actor, input, &actor_password)
                    }),
            )
        }
        "accountManager/users/update" => {
            let input = req
                .params
                .clone()
                .map(serde_json::from_value::<crate::AppUserUpdateInput>)
                .transpose()
                .map_err(|err| format!("invalid user payload: {err}"));
            super::value_or_error(
                input
                    .and_then(|input| input.ok_or_else(|| "missing user payload".to_string()))
                    .and_then(crate::update_app_user),
            )
        }
        "accountManager/users/delete" => {
            let user_id = super::str_param(req, "id").unwrap_or("");
            super::ok_or_error(crate::delete_app_user(user_id))
        }
        "accountManager/users/totp/reset" => {
            let user_id = super::str_param(req, "userId").unwrap_or("");
            super::ok_or_error(crate::reset_app_user_totp(actor, user_id))
        }
        "accountManager/wallet/topUp" => {
            let owner_kind = super::str_param(req, "ownerKind").unwrap_or("user");
            let owner_id = super::str_param(req, "ownerId").unwrap_or("");
            let amount = super::i64_param(req, "amountCreditMicros").unwrap_or(0);
            let note = super::str_param(req, "note");
            let created_by = super::str_param(req, "createdByUserId");
            super::value_or_error(crate::wallet_top_up(
                owner_kind, owner_id, amount, note, created_by,
            ))
        }
        "accountManager/wallet/setAvailable" => {
            let owner_kind = super::str_param(req, "ownerKind").unwrap_or("user");
            let owner_id = super::str_param(req, "ownerId").unwrap_or("");
            let amount = super::i64_param(req, "availableCreditMicros").unwrap_or(0);
            let note = super::str_param(req, "note");
            let created_by = super::str_param(req, "createdByUserId");
            super::value_or_error(crate::wallet_set_available_credit(
                owner_kind, owner_id, amount, note, created_by,
            ))
        }
        "accountManager/apiKeyOwners/list" => super::value_or_error(crate::list_api_key_owners()),
        "accountManager/apiKeyOwners/set" => {
            let key_id = super::str_param(req, "keyId").unwrap_or("");
            let owner_kind = super::str_param(req, "ownerKind").unwrap_or("user");
            let owner_user_id = super::str_param(req, "ownerUserId");
            let project_id = super::str_param(req, "projectId");
            super::value_or_error(crate::set_api_key_owner(
                key_id,
                owner_kind,
                owner_user_id,
                project_id,
            ))
        }
        "accountManager/webAuthMode/set" => {
            let mode = super::str_param(req, "mode").unwrap_or("none");
            super::value_or_error(
                crate::set_web_auth_mode(mode).map(|mode| serde_json::json!({ "mode": mode })),
            )
        }
        "accountManager/distribution/set" => {
            let enabled = super::bool_param(req, "enabled").unwrap_or(false);
            super::value_or_error(
                crate::set_distribution_enabled(enabled)
                    .map(|enabled| serde_json::json!({ "distributionEnabled": enabled })),
            )
        }
        _ => return None,
    };

    Some(super::response(req, result))
}
