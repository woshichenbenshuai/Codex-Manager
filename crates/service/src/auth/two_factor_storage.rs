use codexmanager_core::storage::WebTwoFactorState;
use codexmanager_storage_seaorm::WebTwoFactorRepository;

pub(super) fn load() -> Result<WebTwoFactorState, String> {
    if crate::storage_helpers::seaorm_enabled() {
        return crate::storage_helpers::seaorm_block_on(|storage| async move {
            WebTwoFactorRepository::state(storage.connection())
                .await
                .map_err(|e| e.to_string())
        });
    }
    crate::storage_helpers::open_storage()
        .ok_or("数据库不可用")?
        .web_two_factor_state()
        .map_err(|e| e.to_string())
}

pub(super) fn compare_exchange(version: i64, value: String) -> Result<bool, String> {
    if crate::storage_helpers::seaorm_enabled() {
        return crate::storage_helpers::seaorm_block_on(move |storage| async move {
            WebTwoFactorRepository::compare_exchange(storage.connection(), version, value)
                .await
                .map_err(|e| e.to_string())
        });
    }
    crate::storage_helpers::open_storage()
        .ok_or("数据库不可用")?
        .compare_exchange_web_two_factor_state(version, &value)
        .map_err(|e| e.to_string())
}

pub(super) fn password_hash(actor: &crate::RpcActor) -> Result<String, String> {
    if !actor.is_admin() {
        return Err("permission_denied: 2FA".into());
    }
    if let Some(id) = &actor.user_id {
        let user = if crate::storage_helpers::seaorm_enabled() {
            let id = id.clone();
            crate::storage_helpers::seaorm_block_on(move |s| async move {
                codexmanager_storage_seaorm::UsersRepository::get(s.connection(), &id)
                    .await
                    .map_err(|e| e.to_string())
            })?
        } else {
            crate::storage_helpers::open_storage()
                .ok_or("数据库不可用")?
                .find_app_user_by_id(id)
                .map_err(|e| e.to_string())?
        }
        .filter(|u| u.status == "active" && u.role == "admin")
        .ok_or("管理员账号不可用")?;
        return Ok(user.password_hash);
    }
    if crate::current_web_auth_mode() == "accounts" {
        // A trusted desktop RPC has no Web user ID. In the single-admin case,
        // verify that administrator's existing password without creating users.
        let users = if crate::storage_helpers::seaorm_enabled() {
            crate::storage_helpers::seaorm_block_on(|s| async move {
                codexmanager_storage_seaorm::UsersRepository::list(s.connection())
                    .await
                    .map_err(|e| e.to_string())
            })?
        } else {
            crate::storage_helpers::open_storage()
                .ok_or("数据库不可用")?
                .list_app_users()
                .map_err(|e| e.to_string())?
        };
        let mut admins = users
            .into_iter()
            .filter(|u| u.role == "admin" && u.status == "active");
        let only = admins.next().ok_or("请先初始化管理员账号")?;
        if admins.next().is_some() {
            return Err("请在 Web 中使用对应管理员账号配置 2FA".into());
        }
        return Ok(only.password_hash);
    }
    if crate::current_web_auth_mode() != "password" {
        return Err("请先启用并保存管理登录密码".into());
    }
    crate::current_web_access_password_hash()
        .filter(|h| !h.is_empty())
        .ok_or("请先设置访问密码".into())
}
