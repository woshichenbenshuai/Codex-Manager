use super::*;

use axum::extract::{ConnectInfo, Query};
use serde::Deserialize;
use std::net::SocketAddr;

const WEB_AUTH_TAB_SESSION_STORAGE_KEY: &str = "codexmanager_web_auth_tab";

#[derive(Debug, Deserialize)]
pub(super) struct LoginForm {
    username: Option<String>,
    password: Option<String>,
    display_name: Option<String>,
    totp_code: Option<String>,
    challenge_token: Option<String>,
    flow: Option<String>,
    legacy_password: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct LoginQuery {
    force: Option<String>,
}

/// 函数 `current_web_access_password_hash`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn current_web_access_password_hash() -> Option<String> {
    codexmanager_service::current_web_access_password_hash()
}

/// 函数 `parse_cookie_value`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) fn parse_cookie_value(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|segment| {
        let (name, value) = segment.trim().split_once('=')?;
        if name.trim() == cookie_name {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

/// 函数 `set_cookie_header_value`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - value: 参数 value
///
/// # 返回
/// 返回函数执行结果
fn set_cookie_header_value(value: &str, secure: bool) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "{WEB_AUTH_COOKIE_NAME}={value}; Path=/; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    ))
    .ok()
}

/// 函数 `clear_cookie_header_value`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn clear_cookie_header_value(secure: bool) -> Option<HeaderValue> {
    HeaderValue::from_str(&format!(
        "{WEB_AUTH_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    ))
    .ok()
}

/// 函数 `append_no_store_headers`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - response: 参数 response
///
/// # 返回
/// 无
fn append_no_store_headers(response: &mut Response) {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
        .headers_mut()
        .insert(header::EXPIRES, HeaderValue::from_static("0"));
}

/// 函数 `login_force_requested`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - query: 参数 query
///
/// # 返回
/// 返回函数执行结果
fn login_force_requested(query: &LoginQuery) -> bool {
    query
        .force
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}

fn is_public_static_asset_path(path: &str) -> bool {
    path.starts_with("/_next/")
        || path.starts_with("/static/")
        || path == "/favicon.ico"
        || path == "/robots.txt"
        || path == "/manifest.json"
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".map")
        || path.ends_with(".png")
        || path.ends_with(".jpg")
        || path.ends_with(".jpeg")
        || path.ends_with(".webp")
        || path.ends_with(".svg")
        || path.ends_with(".ico")
        || path.ends_with(".woff")
        || path.ends_with(".woff2")
}

/// 函数 `request_is_authenticated`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - headers: 参数 headers
/// - state: 参数 state
///
/// # 返回
/// 返回函数执行结果
fn request_is_authenticated(headers: &HeaderMap, state: &AppState) -> bool {
    if accounts_mode() {
        return current_app_session_from_headers(headers).is_some();
    }
    let _ = (headers, state);
    codexmanager_service::current_web_auth_mode() == "none"
}

pub(super) fn current_app_session_from_headers(
    headers: &HeaderMap,
) -> Option<codexmanager_service::AppSessionUserResult> {
    if !accounts_mode() {
        return None;
    }
    parse_cookie_value(headers, WEB_AUTH_COOKIE_NAME).and_then(|token| {
        codexmanager_service::resolve_app_user_session(&token)
            .ok()
            .flatten()
    })
}

pub(super) fn accounts_mode() -> bool {
    let mode = codexmanager_service::current_web_auth_mode();
    accounts_mode_from_status(&mode, codexmanager_service::app_auth_status_value())
}

fn accounts_mode_from_status(mode: &str, status: Result<serde_json::Value, String>) -> bool {
    if mode == "accounts" {
        return true;
    }
    match status {
        Ok(value) => value
            .get("appUsersConfigured")
            .and_then(|configured| configured.as_bool())
            .unwrap_or(false),
        // Authentication state is a security boundary. A storage/configuration
        // failure must never downgrade an existing account deployment to the
        // unauthenticated mode.
        Err(_) => true,
    }
}

fn request_is_secure(headers: &HeaderMap) -> bool {
    let _ = headers;
    configured_public_origin().is_some()
}

fn configured_public_origin() -> Option<String> {
    read_env_trim("CODEXMANAGER_WEB_PUBLIC_BASE_URL")
        .as_deref()
        .and_then(public_https_origin)
}

fn request_origin(headers: &HeaderMap) -> Option<String> {
    if let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    {
        return Some(origin.trim_end_matches('/').to_ascii_lowercase());
    }
    let referer = headers.get(header::REFERER)?.to_str().ok()?;
    let (scheme, rest) = referer.split_once("://")?;
    let authority = rest.split('/').next()?;
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    ))
}

fn local_request_origin(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(header::HOST)?.to_str().ok()?.trim();
    web_addr_is_loopback(host).then(|| format!("http://{}", host.to_ascii_lowercase()))
}

fn expected_request_origin(headers: &HeaderMap) -> Option<String> {
    configured_public_origin().or_else(|| local_request_origin(headers))
}

pub(super) async fn csrf_origin_middleware(request: Request, next: Next) -> Response {
    if matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        return next.run(request).await;
    }
    let expected = expected_request_origin(request.headers());
    if expected.is_none() || request_origin(request.headers()) != expected {
        let mut response = (StatusCode::FORBIDDEN, "invalid request origin").into_response();
        append_no_store_headers(&mut response);
        return response;
    }
    next.run(request).await
}

pub(super) async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let sensitive = path.starts_with("/__")
        || path == "/api/rpc"
        || path == "/metrics"
        || path.starts_with("/api/events/");
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        axum::http::HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );
    if request_is_secure(response.headers()) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    if sensitive {
        append_no_store_headers(&mut response);
    }
    response
}

fn trusted_proxy_contains(peer: std::net::IpAddr) -> bool {
    read_env_trim("CODEXMANAGER_WEB_TRUSTED_PROXY_CIDRS")
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .any(|cidr| ip_in_cidr(peer, &cidr))
}

fn ip_in_cidr(peer: std::net::IpAddr, cidr: &str) -> bool {
    let Some((network, prefix)) = cidr.split_once('/') else {
        return cidr.parse::<std::net::IpAddr>().ok() == Some(peer);
    };
    let Ok(network) = network.parse::<std::net::IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u32>() else {
        return false;
    };
    match (peer, network) {
        (std::net::IpAddr::V4(peer), std::net::IpAddr::V4(network)) if prefix <= 32 => {
            let mask = if prefix == 0 {
                0
            } else {
                u32::MAX << (32 - prefix)
            };
            (u32::from(peer) & mask) == (u32::from(network) & mask)
        }
        (std::net::IpAddr::V6(peer), std::net::IpAddr::V6(network)) if prefix <= 128 => {
            let mask = if prefix == 0 {
                0
            } else {
                u128::MAX << (128 - prefix)
            };
            (u128::from(peer) & mask) == (u128::from(network) & mask)
        }
        _ => false,
    }
}

fn authentication_source_ip(peer: SocketAddr, headers: &HeaderMap) -> String {
    if trusted_proxy_contains(peer.ip()) {
        if let Some(forwarded) = forwarded_client_ip(headers) {
            return forwarded.to_string();
        }
    }
    peer.ip().to_string()
}

fn forwarded_client_ip(headers: &HeaderMap) -> Option<std::net::IpAddr> {
    if let Some(forwarded) = headers
        .get("forwarded")
        .and_then(|value| value.to_str().ok())
    {
        let value = forwarded
            .split(',')
            .next_back()?
            .split(';')
            .find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                name.eq_ignore_ascii_case("for").then_some(value.trim())
            })?
            .trim_matches('"');
        if let Some(ip) = parse_forwarded_ip(value) {
            return Some(ip);
        }
    }
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next_back())
        .map(str::trim)
        .and_then(parse_forwarded_ip)
}

fn parse_forwarded_ip(value: &str) -> Option<std::net::IpAddr> {
    value
        .parse::<std::net::IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|addr| addr.ip()))
}

fn account_auth_html(
    error: Option<&str>,
    bootstrap: bool,
    flow: Option<&str>,
    challenge_token: Option<&str>,
    setup: Option<&codexmanager_service::AppUserTotpSetupResult>,
) -> String {
    let error_html = error
        .map(|text| format!(r#"<div class="error">{}</div>"#, escape_html(text)))
        .unwrap_or_default();
    let setup_mode = setup.is_some();
    let setup_flow = setup_mode || flow == Some("setup");
    let title = if setup_mode {
        "绑定验证器"
    } else if setup_flow {
        "确认验证器"
    } else if bootstrap {
        "初始化管理员"
    } else if flow == Some("login") {
        "输入验证器验证码"
    } else {
        "账号登录"
    };
    let description = if setup_mode {
        "请将下面的密钥添加到 Google Authenticator、Microsoft Authenticator 或其他 TOTP 应用，然后输入 6 位验证码完成绑定。"
    } else if setup_flow {
        "请重新输入验证器应用当前显示的 6 位动态验证码。密钥不会再次显示。"
    } else if bootstrap {
        "请创建管理员账号。管理员必须完成验证器绑定后才能进入控制台。"
    } else if flow == Some("login") {
        "请输入验证器应用当前显示的 6 位动态验证码。"
    } else {
        "请输入账户名和密码登录。"
    };
    let setup_html = setup
        .map(|value| {
            format!(
                r#"<div class="setup"><div><strong>验证器密钥</strong></div><code>{}</code><div class="uri">{}</div></div>"#,
                escape_html(&value.secret),
                escape_html(&value.otpauth_uri)
            )
        })
        .unwrap_or_default();
    let credential_html = if setup_flow || flow == Some("login") {
        String::new()
    } else {
        format!(
            r#"<label for="username">账户名</label>
      <input id="username" name="username" type="text" autocomplete="username" autofocus />
      {}
      {}
      <label for="password">密码</label>
      <input id="password" name="password" type="password" autocomplete="current-password" />"#,
            if bootstrap {
                r#"<label for="display_name">显示名称</label>
      <input id="display_name" name="display_name" type="text" autocomplete="name" />"#
            } else {
                ""
            },
            if bootstrap {
                r#"<label for="legacy_password">旧访问密码 / Bootstrap 密码</label>
      <input id="legacy_password" name="legacy_password" type="password" autocomplete="current-password" />"#
            } else {
                ""
            }
        )
    };
    let hidden = challenge_token
        .map(|token| {
            format!(
                r#"<input type="hidden" name="challenge_token" value="{}" />
      <input type="hidden" name="flow" value="{}" />"#,
                escape_html(token),
                escape_html(flow.unwrap_or("setup"))
            )
        })
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><title>CodexManager 登录</title>
<style>body{{margin:0;min-height:100vh;display:grid;place-items:center;padding:24px;font-family:Segoe UI,PingFang SC,Microsoft YaHei,sans-serif;background:#eef3f8;color:#142033}}.card{{width:min(100%,460px);padding:28px;border:1px solid #d5deea;border-radius:20px;background:#fff;box-shadow:0 24px 60px #0f172a1f}}h1{{margin:16px 0 6px;font-size:22px}}p{{margin:0 0 18px;color:#627389;line-height:1.6}}label{{display:block;margin:14px 0 8px;font-size:14px;color:#627389}}input{{width:100%;box-sizing:border-box;border:1px solid #cbd5e1;border-radius:12px;padding:12px 14px;font-size:15px}}button{{width:100%;margin-top:18px;border:0;border-radius:12px;padding:13px 16px;font-size:15px;font-weight:600;color:#fff;background:#0f6fff;cursor:pointer}}.error{{margin-bottom:14px;padding:12px 14px;border-radius:12px;background:#fee2e2;color:#b42318;font-size:14px}}.setup{{margin:14px 0;padding:14px;border-radius:12px;background:#f1f5f9;line-height:1.8}}code,.uri{{display:block;word-break:break-all;margin-top:6px}}.uri{{font-size:12px;color:#627389}}</style></head><body><form class="card" method="post" action="/__login"><div>CM</div><h1>{title}</h1><p>{description}</p>{error_html}{hidden}{credential_html}{setup_html}<label for="totp_code">{code_label}</label><input id="totp_code" name="totp_code" inputmode="numeric" pattern="[0-9]{{6}}" maxlength="6" autocomplete="one-time-code" autofocus /><button type="submit">{button}</button></form></body></html>"#,
        title = title,
        description = description,
        error_html = error_html,
        hidden = hidden,
        credential_html = credential_html,
        setup_html = setup_html,
        code_label = if setup_flow {
            "验证器验证码"
        } else {
            "动态验证码（未绑定时留空）"
        },
        button = if setup_flow { "完成绑定" } else { "登录" },
    )
}

fn legacy_password_migration_html(error: Option<&str>) -> String {
    let error_html = error
        .map(|text| format!(r#"<div class="error">{}</div>"#, escape_html(text)))
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><title>迁移 CodexManager 账号</title><style>body{{margin:0;min-height:100vh;display:grid;place-items:center;padding:24px;font-family:Segoe UI,PingFang SC,Microsoft YaHei,sans-serif;background:#eef3f8;color:#142033}}.card{{width:min(100%,460px);padding:28px;border-radius:20px;background:#fff;box-shadow:0 24px 60px #0f172a1f}}label{{display:block;margin:14px 0 8px;color:#627389}}input{{width:100%;box-sizing:border-box;padding:12px;border:1px solid #cbd5e1;border-radius:12px}}button{{width:100%;margin-top:18px;padding:13px;border:0;border-radius:12px;background:#0f6fff;color:#fff;font-weight:600}}.error{{margin-bottom:14px;padding:12px;border-radius:12px;background:#fee2e2;color:#b42318}}</style></head><body><form class="card" method="post" action="/__login"><h1>初始化账户登录</h1><p>旧版本访问密码只用于一次性迁移，请创建新的管理员账户并绑定验证器。</p>{error_html}<label>旧访问密码</label><input name="legacy_password" type="password" autocomplete="current-password" autofocus/><label>新管理员账户名</label><input name="username" type="text" autocomplete="username"/><label>新管理员密码</label><input name="password" type="password" autocomplete="new-password"/><label>显示名称</label><input name="display_name" type="text" autocomplete="name"/><button type="submit">开始迁移</button></form></body></html>"#,
        error_html = error_html,
    )
}

/// 函数 `login_success_html`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn login_success_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <title>CodexManager Web 登录</title>
  </head>
  <body>
    <script>
      try {{
        window.sessionStorage.setItem("{WEB_AUTH_TAB_SESSION_STORAGE_KEY}", "1");
      }} catch (_err) {{}}
      window.location.replace("/");
    </script>
  </body>
</html>
"#
    )
}

/// 函数 `logout_success_html`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 返回函数执行结果
fn logout_success_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <title>CodexManager Web 已退出</title>
  </head>
  <body>
    <script>
      try {{
        window.sessionStorage.removeItem("{WEB_AUTH_TAB_SESSION_STORAGE_KEY}");
      }} catch (_err) {{}}
      window.location.replace("/__login?force=1");
    </script>
  </body>
</html>
"#
    )
}

/// 函数 `web_auth_middleware`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) async fn web_auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if path == "/__login" || path == "/__logout" || is_public_static_asset_path(&path) {
        return next.run(request).await;
    }
    if request_is_authenticated(request.headers(), state.as_ref()) {
        return next.run(request).await;
    }
    if path.starts_with("/api/") {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({ "error": "web_auth_required" })),
        )
            .into_response();
    }
    Redirect::to("/__login").into_response()
}

/// 函数 `login_page`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) async fn login_page(
    State(state): State<Arc<AppState>>,
    Query(query): Query<LoginQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let mode = codexmanager_service::current_web_auth_mode();
    if mode == "none" && !accounts_mode() {
        return Redirect::to("/").into_response();
    }
    if request_is_authenticated(&headers, state.as_ref()) && !login_force_requested(&query) {
        return Redirect::to("/").into_response();
    }
    let html = if accounts_mode() {
        let bootstrap = codexmanager_service::app_auth_status_value()
            .ok()
            .and_then(|value| {
                value
                    .get("appUsersConfigured")
                    .and_then(|configured| configured.as_bool())
                    .map(|configured| !configured)
            })
            .unwrap_or(true);
        account_auth_html(None, bootstrap, None, None, None)
    } else {
        legacy_password_migration_html(None)
    };
    let mut response = Html(html).into_response();
    append_no_store_headers(&mut response);
    response
}

/// 函数 `login_submit`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) async fn login_submit(
    State(_state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Form(form): axum::Form<LoginForm>,
) -> impl IntoResponse {
    let mode = codexmanager_service::current_web_auth_mode();
    if mode == "none" && !accounts_mode() {
        return Redirect::to("/").into_response();
    }
    if accounts_mode() {
        let bootstrap = codexmanager_service::app_auth_status_value()
            .ok()
            .and_then(|value| {
                value
                    .get("appUsersConfigured")
                    .and_then(|configured| configured.as_bool())
                    .map(|configured| !configured)
            })
            .unwrap_or(true);
        if form.flow.as_deref() == Some("setup") {
            let result = codexmanager_service::confirm_app_user_totp_setup(
                None,
                form.challenge_token.as_deref(),
                form.totp_code.as_deref().unwrap_or(""),
            );
            return match result {
                Ok(login) => {
                    let mut response = Html(login_success_html()).into_response();
                    if let Some(header_value) =
                        set_cookie_header_value(&login.token, request_is_secure(&headers))
                    {
                        response
                            .headers_mut()
                            .append(header::SET_COOKIE, header_value);
                    }
                    append_no_store_headers(&mut response);
                    response
                }
                Err(err) => {
                    let challenge_is_retryable = form
                        .challenge_token
                        .as_deref()
                        .filter(|value| !value.trim().is_empty())
                        .filter(|_| {
                            !err.contains("过期")
                                && !err.contains("失效")
                                && !err.contains("次数过多")
                                && !err.contains("已使用")
                        });
                    let mut response = (
                        StatusCode::UNAUTHORIZED,
                        Html(account_auth_html(
                            Some(&err),
                            false,
                            challenge_is_retryable.map(|_| "setup"),
                            challenge_is_retryable,
                            None,
                        )),
                    )
                        .into_response();
                    append_no_store_headers(&mut response);
                    response
                }
            };
        }
        let username = form.username.as_deref().unwrap_or("");
        let password = form.password.as_deref().unwrap_or("");
        let result = if bootstrap {
            codexmanager_service::bootstrap_app_admin_with_totp(
                form.legacy_password.as_deref().unwrap_or(""),
                username,
                password,
                form.display_name.as_deref(),
            )
        } else {
            codexmanager_service::login_app_user_from_source(
                username,
                password,
                form.totp_code.as_deref(),
                form.challenge_token.as_deref(),
                Some(&authentication_source_ip(peer, &headers)),
            )
        };
        match result {
            Ok(codexmanager_service::AppLoginAttempt::Authenticated(login)) => {
                let mut response = Html(login_success_html()).into_response();
                if let Some(header_value) =
                    set_cookie_header_value(&login.token, request_is_secure(&headers))
                {
                    response
                        .headers_mut()
                        .append(header::SET_COOKIE, header_value);
                }
                append_no_store_headers(&mut response);
                return response;
            }
            Ok(codexmanager_service::AppLoginAttempt::TotpChallenge {
                challenge_token,
                error,
                ..
            }) => {
                let challenge_is_retryable = !error.as_deref().is_some_and(|message| {
                    message.contains("过期")
                        || message.contains("失效")
                        || message.contains("次数过多")
                        || message.contains("已使用")
                });
                let mut response = (
                    StatusCode::UNAUTHORIZED,
                    Html(account_auth_html(
                        error.as_deref(),
                        false,
                        challenge_is_retryable.then_some("login"),
                        challenge_is_retryable.then_some(challenge_token.as_str()),
                        None,
                    )),
                )
                    .into_response();
                append_no_store_headers(&mut response);
                return response;
            }
            Ok(codexmanager_service::AppLoginAttempt::TotpSetup {
                challenge_token,
                setup,
            }) => {
                let mut response = (
                    StatusCode::UNAUTHORIZED,
                    Html(account_auth_html(
                        None,
                        false,
                        Some("setup"),
                        Some(&challenge_token),
                        Some(&setup),
                    )),
                )
                    .into_response();
                append_no_store_headers(&mut response);
                return response;
            }
            Err(err) => {
                let challenge_is_retryable = form
                    .challenge_token
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .filter(|_| {
                        !err.contains("过期")
                            && !err.contains("失效")
                            && !err.contains("次数过多")
                            && !err.contains("已使用")
                    });
                let mut response = (
                    StatusCode::UNAUTHORIZED,
                    Html(account_auth_html(
                        Some(&err),
                        bootstrap,
                        challenge_is_retryable
                            .map(|_| "login")
                            .or(form.flow.as_deref()),
                        challenge_is_retryable,
                        None,
                    )),
                )
                    .into_response();
                append_no_store_headers(&mut response);
                return response;
            }
        }
    }
    if current_web_access_password_hash().is_none() {
        return Redirect::to("/").into_response();
    }
    let legacy_password = form.legacy_password.as_deref().unwrap_or("");
    if form.username.as_deref().unwrap_or("").trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Html(legacy_password_migration_html(Some(
                "请输入新的管理员账户名和密码。",
            ))),
        )
            .into_response();
    }
    match codexmanager_service::bootstrap_app_admin_with_totp(
        legacy_password,
        form.username.as_deref().unwrap_or(""),
        form.password.as_deref().unwrap_or(""),
        form.display_name.as_deref(),
    ) {
        Ok(codexmanager_service::AppLoginAttempt::TotpSetup {
            challenge_token,
            setup,
        }) => {
            let mut response = Html(account_auth_html(
                None,
                false,
                Some("setup"),
                Some(&challenge_token),
                Some(&setup),
            ))
            .into_response();
            append_no_store_headers(&mut response);
            return response;
        }
        Ok(codexmanager_service::AppLoginAttempt::Authenticated(_)) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(legacy_password_migration_html(Some(
                    "迁移流程未完成验证器绑定，请重新开始。",
                ))),
            )
                .into_response();
        }
        Ok(codexmanager_service::AppLoginAttempt::TotpChallenge { .. }) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(legacy_password_migration_html(Some(
                    "迁移流程返回了无效的验证器状态，请重新开始。",
                ))),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Html(legacy_password_migration_html(Some(&err))),
            )
                .into_response();
        }
    }
}

/// 函数 `logout`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) async fn logout(headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = parse_cookie_value(&headers, WEB_AUTH_COOKIE_NAME) {
        let _ = codexmanager_service::logout_app_user_session(&token);
    }
    let mut response = Html(logout_success_html()).into_response();
    if let Some(header_value) = clear_cookie_header_value(request_is_secure(&headers)) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, header_value);
    }
    append_no_store_headers(&mut response);
    response
}

/// 函数 `auth_status`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// - super: 参数 super
///
/// # 返回
/// 返回函数执行结果
pub(super) async fn auth_status(headers: HeaderMap) -> impl IntoResponse {
    let mut status = codexmanager_service::app_auth_status_value().unwrap_or_else(|_| {
        serde_json::json!({
            "mode": codexmanager_service::current_web_auth_mode(),
            "passwordConfigured": current_web_access_password_hash().is_some(),
            "appUsersConfigured": false,
            "distributionEnabled": false,
            "billingModeLock": {
                "accountModeLocked": false,
                "distributionLocked": false,
                "reasons": []
            },
        })
    });
    let session = current_app_session_from_headers(&headers);
    if session.is_none() {
        let mode = status
            .get("mode")
            .and_then(|value| value.as_str())
            .unwrap_or("none");
        let bootstrap_required = !status
            .get("appUsersConfigured")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let public_status = serde_json::json!({
            "mode": mode,
            "requiresAuth": mode != "none" || !bootstrap_required,
            "bootstrapRequired": bootstrap_required,
        });
        let mut response = axum::Json(public_status).into_response();
        append_no_store_headers(&mut response);
        return response;
    }
    let actor = session
        .as_ref()
        .map(|session| {
            codexmanager_service::RpcActor::from_parts_with_session(
                Some(session.user.role.as_str()),
                Some(session.user.id.as_str()),
                Some(session.session_id.as_str()),
            )
        })
        .unwrap_or_else(codexmanager_service::RpcActor::system_admin);
    restrict_authenticated_auth_status(&mut status, &actor.role);
    if let Some(object) = status.as_object_mut() {
        if let Some(session) = session {
            object.insert(
                "currentUser".to_string(),
                serde_json::to_value(session.user).unwrap_or(serde_json::Value::Null),
            );
        }
        object.insert("role".to_string(), serde_json::json!(actor.role));
        object.insert(
            "permissions".to_string(),
            serde_json::json!(actor
                .permissions()
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()),
        );
    }
    let mut response = axum::Json(status).into_response();
    append_no_store_headers(&mut response);
    response
}

fn restrict_authenticated_auth_status(status: &mut serde_json::Value, role: &str) {
    if matches!(
        role,
        codexmanager_service::ROLE_ADMIN | codexmanager_service::ROLE_SYSTEM_ADMIN
    ) {
        return;
    }
    let Some(object) = status.as_object_mut() else {
        return;
    };
    for key in [
        "modeOptions",
        "passwordConfigured",
        "appUsersConfigured",
        "appUserCount",
        "activeAdminCount",
        "billingModeLock",
    ] {
        object.remove(key);
    }
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
