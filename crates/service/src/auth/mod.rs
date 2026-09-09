#[path = "auth_account.rs"]
pub(crate) mod account;
pub(crate) mod app_manager;
#[path = "auth_callback.rs"]
pub(crate) mod callback;
#[path = "auth_login.rs"]
pub(crate) mod login;
pub(crate) mod rpc;
#[path = "auth_tokens.rs"]
pub(crate) mod tokens;
pub(crate) mod totp;
pub(crate) mod web_access;

pub use app_manager::app_auth_status_value as web_auth_status_value;
pub use app_manager::validate_web_totp_configuration as validate_web_totp_encryption_key;
pub use app_manager::{
    api_key_belongs_to_user, app_auth_status_value, app_session_result, app_user_totp_status,
    begin_app_user_totp_setup, billing_mode_lock_status, bootstrap_app_admin,
    bootstrap_app_admin_with_totp, change_app_user_password, complete_app_login_challenge,
    confirm_app_user_totp_setup, confirm_app_user_totp_setup_for_actor, create_app_user,
    create_app_user_for_actor, current_web_auth_mode, delete_app_user, disable_app_user_totp,
    distribution_enabled, list_api_key_ids_for_user, list_api_key_owners, list_app_user_sessions,
    list_app_users, login_app_user, login_app_user_from_source, logout_app_user_session,
    record_request_charge_v2, reset_app_user_totp, resolve_app_user_session,
    revoke_app_user_session, set_api_key_owner, set_distribution_enabled, set_web_auth_mode,
    update_app_user, update_app_user_profile, wallet_precheck_for_api_key,
    wallet_set_available_credit, wallet_top_up, ApiKeyOwnerResult, AppLoginAttempt, AppLoginResult,
    AppSessionResult, AppSessionUserResult, AppUserCreateInput, AppUserCreateResult,
    AppUserPublicResult, AppUserSessionResult, AppUserTotpSetupResult, AppUserTotpStatusResult,
    AppUserUpdateInput, AppWalletResult, BillingModeLockResult,
};
pub use rpc::{rpc_auth_token, rpc_auth_token_matches};
pub use web_access::{
    bootstrap_web_access_password_if_missing, current_web_access_password_hash,
    set_web_access_password, verify_web_access_password, web_access_password_configured,
};
