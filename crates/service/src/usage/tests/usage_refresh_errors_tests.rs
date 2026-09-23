use super::{
    classify_usage_refresh_error, should_record_failure_event_with_state,
    status_reason_for_refresh_failure, FailureThrottleKey,
};
use std::collections::HashMap;

use codexmanager_core::storage::{now_ts, Account, Event, Storage};

/// 函数 `usage_refresh_error_class_groups_by_status_code`
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
fn usage_refresh_error_class_groups_by_status_code() {
    assert_eq!(
        classify_usage_refresh_error("usage endpoint status 500 Internal Server Error"),
        "usage_status_500"
    );
    assert_eq!(
        classify_usage_refresh_error("usage endpoint status 503 Service Unavailable"),
        "usage_status_503"
    );
    assert_eq!(
        classify_usage_refresh_error("subscription endpoint status 401 Unauthorized"),
        "usage_status_401"
    );
    assert_eq!(
        classify_usage_refresh_error(
            "subscription endpoint failed: status=503 Service Unavailable body=upstream unavailable"
        ),
        "usage_status_503"
    );
}

/// 函数 `usage_refresh_error_class_catches_timeout_and_connection`
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
fn usage_refresh_error_class_catches_timeout_and_connection() {
    assert_eq!(
        classify_usage_refresh_error("request timeout while calling usage"),
        "timeout"
    );
    assert_eq!(
        classify_usage_refresh_error("connection reset by peer"),
        "connection"
    );
    assert_eq!(
        classify_usage_refresh_error(
            "error sending request for url (https://chatgpt.com/backend-api/accounts/check)"
        ),
        "connection"
    );
    assert_eq!(classify_usage_refresh_error("unknown error"), "other");
}

/// 函数 `usage_refresh_error_class_maps_to_visible_status_reason`
///
/// 作者: gaohongshun
///
/// 时间: 2026-06-23
///
/// # 参数
/// 无
///
/// # 返回
/// 无
#[test]
fn usage_refresh_error_class_maps_to_visible_status_reason() {
    assert_eq!(
        status_reason_for_refresh_failure("timeout"),
        Some("usage_refresh_timeout")
    );
    assert_eq!(
        status_reason_for_refresh_failure("connection"),
        Some("usage_refresh_connection")
    );
    assert_eq!(
        status_reason_for_refresh_failure("dns"),
        Some("usage_refresh_dns")
    );
    assert_eq!(
        status_reason_for_refresh_failure("other"),
        Some("usage_refresh_failed")
    );
    assert_eq!(status_reason_for_refresh_failure("usage_status_500"), None);
}

/// 函数 `failure_event_throttle_dedupes_within_window`
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
fn failure_event_throttle_dedupes_within_window() {
    let mut state = HashMap::new();
    let key = FailureThrottleKey {
        account_id: "acc-1".to_string(),
        error_class: "usage_status_500".to_string(),
    };

    assert!(should_record_failure_event_with_state(
        &mut state,
        key.clone(),
        100,
        60
    ));
    assert!(!should_record_failure_event_with_state(
        &mut state,
        key.clone(),
        120,
        60
    ));
    assert!(should_record_failure_event_with_state(
        &mut state, key, 161, 60
    ));
}

/// 函数 `failure_event_throttle_isolated_by_error_class`
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
fn failure_event_throttle_isolated_by_error_class() {
    let mut state = HashMap::new();
    let key_500 = FailureThrottleKey {
        account_id: "acc-1".to_string(),
        error_class: "usage_status_500".to_string(),
    };
    let key_timeout = FailureThrottleKey {
        account_id: "acc-1".to_string(),
        error_class: "timeout".to_string(),
    };

    assert!(should_record_failure_event_with_state(
        &mut state, key_500, 100, 60
    ));
    assert!(should_record_failure_event_with_state(
        &mut state,
        key_timeout,
        110,
        60
    ));
}

#[test]
fn refresh_failure_preserves_confirmed_usage_limit_reason() {
    let storage = Storage::open_in_memory().expect("open storage");
    storage.init().expect("init storage");
    let now = now_ts();
    let account_id = format!("usage-limited-{}", std::process::id());
    storage
        .insert_account(&Account {
            id: account_id.clone(),
            label: "limited".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "limited".to_string(),
            created_at: now,
            updated_at: now,
        })
        .expect("insert account");
    storage
        .insert_event(&Event {
            account_id: Some(account_id.clone()),
            event_type: "account_status_update".to_string(),
            message: "status=limited reason=usage_limit_exhausted".to_string(),
            created_at: now,
        })
        .expect("insert confirmed limit event");

    super::record_usage_refresh_failure(&storage, &account_id, "connection reset by peer");

    let reasons = storage
        .latest_account_status_reasons(&[account_id.clone()])
        .expect("load status reason");
    assert_eq!(
        reasons.get(&account_id).map(String::as_str),
        Some("usage_limit_exhausted")
    );
}
