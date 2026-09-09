use super::*;

#[test]
fn member_auth_status_hides_account_inventory_and_legacy_auth_details() {
    let mut status = serde_json::json!({
        "mode": "accounts",
        "modeOptions": ["none", "accounts"],
        "passwordConfigured": true,
        "appUsersConfigured": true,
        "appUserCount": 7,
        "activeAdminCount": 2,
        "distributionEnabled": true,
        "billingModeLock": { "reasons": ["member_users"] }
    });

    restrict_authenticated_auth_status(&mut status, codexmanager_service::ROLE_MEMBER);

    assert_eq!(status["mode"], "accounts");
    assert_eq!(status["distributionEnabled"], true);
    for key in [
        "modeOptions",
        "passwordConfigured",
        "appUsersConfigured",
        "appUserCount",
        "activeAdminCount",
        "billingModeLock",
    ] {
        assert!(status.get(key).is_none(), "member response leaked {key}");
    }
}

#[test]
fn admin_auth_status_keeps_management_details() {
    let mut status = serde_json::json!({
        "appUserCount": 7,
        "activeAdminCount": 2,
        "billingModeLock": { "reasons": [] }
    });

    restrict_authenticated_auth_status(&mut status, codexmanager_service::ROLE_ADMIN);

    assert_eq!(status["appUserCount"], 7);
    assert_eq!(status["activeAdminCount"], 2);
    assert!(status.get("billingModeLock").is_some());
}

/// 函数 `login_force_requested_accepts_truthy_flags`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn login_force_requested_accepts_truthy_flags() {
    for value in ["1", "true", "TRUE", "yes", "on"] {
        let query = LoginQuery {
            force: Some(value.to_string()),
        };
        assert!(login_force_requested(&query), "value={value}");
    }
    for value in ["", "0", "false", "no", "off"] {
        let query = LoginQuery {
            force: Some(value.to_string()),
        };
        assert!(!login_force_requested(&query), "value={value}");
    }
    assert!(!login_force_requested(&LoginQuery::default()));
}

/// 函数 `login_success_html_marks_current_tab_session`
///
/// 作者: gaohongshun
///
/// 时间: 2026-04-02
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn login_success_html_marks_current_tab_session() {
    let html = login_success_html();
    assert!(html.contains("sessionStorage.setItem"));
    assert!(html.contains(WEB_AUTH_TAB_SESSION_STORAGE_KEY));
    assert!(html.contains("location.replace(\"/\")"));
}

#[test]
fn web_auth_allows_static_assets_without_session() {
    for path in [
        "/_next/static/chunks/app/page.js",
        "/_next/static/css/app.css",
        "/favicon.ico",
        "/author-alipay.jpg",
        "/manifest.json",
    ] {
        assert!(is_public_static_asset_path(path), "path={path}");
    }
    assert!(!is_public_static_asset_path("/settings"));
    assert!(!is_public_static_asset_path("/api/rpc"));
}

#[test]
fn trusted_proxy_cidr_matching_supports_ipv4_and_ipv6() {
    assert!(ip_in_cidr("10.2.3.4".parse().unwrap(), "10.0.0.0/8"));
    assert!(!ip_in_cidr("11.2.3.4".parse().unwrap(), "10.0.0.0/8"));
    assert!(ip_in_cidr("2001:db8::42".parse().unwrap(), "2001:db8::/32"));
    assert!(!ip_in_cidr(
        "2001:db9::42".parse().unwrap(),
        "2001:db8::/32"
    ));
}

#[test]
fn local_csrf_origin_fallback_rejects_dns_rebinding_hosts() {
    let mut local = HeaderMap::new();
    local.insert(header::HOST, HeaderValue::from_static("127.0.0.1:48761"));
    assert_eq!(
        local_request_origin(&local).as_deref(),
        Some("http://127.0.0.1:48761")
    );

    let mut rebinding = HeaderMap::new();
    rebinding.insert(header::HOST, HeaderValue::from_static("attacker.example"));
    assert!(local_request_origin(&rebinding).is_none());
}

#[test]
fn forwarded_client_ip_uses_the_nearest_hop_and_parses_socket_addresses() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=198.51.100.1;proto=https, for=\"[2001:db8::7]:443\""),
    );
    assert_eq!(
        forwarded_client_ip(&headers),
        Some("2001:db8::7".parse().unwrap())
    );

    headers.remove("forwarded");
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("198.51.100.1, 203.0.113.9"),
    );
    assert_eq!(
        forwarded_client_ip(&headers),
        Some("203.0.113.9".parse().unwrap())
    );
}

#[test]
fn account_auth_html_preserves_bootstrap_and_setup_retry_states() {
    let bootstrap = account_auth_html(Some("error"), true, None, None, None);
    assert!(bootstrap.contains("name=\"legacy_password\""));
    assert!(bootstrap.contains("name=\"username\""));

    let setup_retry = account_auth_html(
        Some("验证码错误"),
        false,
        Some("setup"),
        Some("challenge-token"),
        None,
    );
    assert!(setup_retry.contains("密钥不会再次显示"));
    assert!(setup_retry.contains("name=\"challenge_token\""));
    assert!(!setup_retry.contains("name=\"username\""));
    assert!(!setup_retry.contains("name=\"password\""));
}

#[test]
fn account_mode_detection_fails_closed_on_status_errors() {
    assert!(accounts_mode_from_status(
        "none",
        Err("database unavailable".to_string())
    ));
    assert!(accounts_mode_from_status(
        "accounts",
        Ok(serde_json::json!({ "appUsersConfigured": false }))
    ));
    assert!(accounts_mode_from_status(
        "none",
        Ok(serde_json::json!({ "appUsersConfigured": true }))
    ));
    assert!(!accounts_mode_from_status(
        "none",
        Ok(serde_json::json!({ "appUsersConfigured": false }))
    ));
}
