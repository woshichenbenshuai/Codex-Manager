use super::*;
use axum::http::HeaderValue;
use codexmanager_service::two_factor;
use serde::Deserialize;

const RECEIPT_COOKIE: &str = "codexmanager_web_2fa";
const CHALLENGE_COOKIE: &str = "codexmanager_2fa_challenge";

fn cookie(name: &str, value: &str, max_age: Option<u32>) -> HeaderValue {
    let age = max_age
        .map(|v| format!("; Max-Age={v}"))
        .unwrap_or_default();
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; HttpOnly; SameSite=Strict{age}"
    ))
    .unwrap()
}

pub(super) fn receipt_allowed(mode: &str, headers: &HeaderMap) -> bool {
    let primary = auth::parse_cookie_value(headers, WEB_AUTH_COOKIE_NAME).unwrap_or_default();
    let receipt = auth::parse_cookie_value(headers, RECEIPT_COOKIE).unwrap_or_default();
    two_factor::receipt_valid(mode, &primary, &receipt).unwrap_or(false)
}

pub(super) fn enrollment_cookie(headers: &HeaderMap) -> Option<HeaderValue> {
    let primary = auth::parse_cookie_value(headers, WEB_AUTH_COOKIE_NAME)?;
    let receipt =
        two_factor::current_receipt(&codexmanager_service::current_web_auth_mode(), &primary)
            .ok()?;
    Some(cookie(RECEIPT_COOKIE, &receipt, None))
}

pub(super) fn clear_cookies(response: &mut Response) {
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie(RECEIPT_COOKIE, "", Some(0)));
    response
        .headers_mut()
        .append(header::SET_COOKIE, cookie(CHALLENGE_COOKIE, "", Some(0)));
}

fn form(challenge: &str, error: Option<&str>) -> String {
    let challenge = challenge
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    let error = error
        .map(|e| {
            format!(
                "<p role=\"alert\">{}</p>",
                e.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('"', "&quot;")
            )
        })
        .unwrap_or_default();
    format!(
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>两步验证 · CodexManager</title><style>body{{font-family:system-ui;background:#f4f6fb;margin:0;min-height:100dvh;display:grid;place-items:center;color:#172033}}form{{box-sizing:border-box;background:white;border:1px solid #dce1ec;border-radius:16px;padding:28px;width:min(420px,calc(100vw - 32px))}}label,input,button{{display:block;box-sizing:border-box;width:100%}}input,button{{padding:12px;margin-top:12px;border:1px solid #c9d2e4;border-radius:8px}}button{{background:#2563eb;color:white;font-weight:600}}p{{font-size:14px;line-height:1.6}}[role=alert]{{color:#b42318}}</style></head><body><form method="post" action="/__2fa"><h1>两步验证</h1><p>密码验证成功。请输入验证器中的 6 位动态验证码，或一条未使用的恢复码。</p>{error}<input type="hidden" name="challenge" value="{challenge}"><label for="code">动态验证码或恢复码</label><input id="code" name="code" autocomplete="one-time-code" maxlength="64" required autofocus><button type="submit">验证并登录</button><p><a href="/__login?force=1">重新输入密码</a></p></form></body></html>"#
    )
}

fn no_store(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
}

pub(super) fn begin(
    mode: &str,
    token: &str,
    user_id: Option<String>,
) -> Result<Option<Response>, String> {
    let Some(challenge) = two_factor::issue_login(mode, token, user_id)? else {
        return Ok(None);
    };
    let mut response = Html(form(&challenge, None)).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        cookie(CHALLENGE_COOKIE, &challenge, Some(300)),
    );
    no_store(&mut response);
    Ok(Some(response))
}

#[derive(Deserialize)]
pub(super) struct VerifyForm {
    challenge: String,
    code: String,
}

fn browser_challenge_allowed(headers: &HeaderMap, challenge: &str) -> bool {
    let same_site = headers
        .get("sec-fetch-site")
        .is_none_or(|v| matches!(v.to_str(), Ok("same-origin" | "none")));
    let origin_allowed = headers.get(header::ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .ok()
            .and_then(|o| o.parse::<axum::http::Uri>().ok())
            .filter(|o| {
                matches!(o.scheme_str(), Some("http" | "https"))
                    && o.path() == "/"
                    && o.query().is_none()
            })
            .and_then(|o| o.authority().map(|a| a.as_str().to_owned()))
            .zip(headers.get(header::HOST).and_then(|h| h.to_str().ok()))
            .is_some_and(|(origin, host)| origin.eq_ignore_ascii_case(host))
    });
    let browser_token = auth::parse_cookie_value(&headers, CHALLENGE_COOKIE).unwrap_or_default();
    same_site
        && origin_allowed
        && !browser_token.is_empty()
        && browser_token == challenge
        && challenge.len() == 64
        && challenge.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(super) async fn complete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Form(input): axum::Form<VerifyForm>,
) -> Response {
    if !browser_challenge_allowed(&headers, &input.challenge) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = two_factor::complete_login(&input.challenge, &input.code);
    match result {
        Ok(login) => {
            // Password cookies are scoped to this Web process's session key.
            if codexmanager_service::current_web_auth_mode() == "password" {
                let expected = auth::build_web_auth_cookie_value(
                    &codexmanager_service::current_web_access_password_hash().unwrap_or_default(),
                    &state.rpc_token,
                    &state.web_auth_session_key,
                );
                if login.token != expected {
                    return Redirect::to("/__login?force=1").into_response();
                }
            }
            let mut response = Html(auth::login_success_html()).into_response();
            response.headers_mut().append(
                header::SET_COOKIE,
                auth::set_cookie_header_value(&login.token).unwrap(),
            );
            response.headers_mut().append(
                header::SET_COOKIE,
                cookie(RECEIPT_COOKIE, &login.receipt, None),
            );
            response
                .headers_mut()
                .append(header::SET_COOKIE, cookie(CHALLENGE_COOKIE, "", Some(0)));
            no_store(&mut response);
            response
        }
        Err(error) => {
            let mut response = (
                StatusCode::UNAUTHORIZED,
                Html(form(&input.challenge, Some(&error))),
            )
                .into_response();
            no_store(&mut response);
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_is_bound_to_the_browser_and_same_origin() {
        let token = "a".repeat(64);
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("manager.example"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://manager.example"),
        );
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{CHALLENGE_COOKIE}={token}")).unwrap(),
        );
        assert!(browser_challenge_allowed(&headers, &token));
        assert!(!browser_challenge_allowed(&headers, &"b".repeat(64)));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://attacker.example"),
        );
        assert!(!browser_challenge_allowed(&headers, &token));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://manager.example"),
        );
        headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!browser_challenge_allowed(&headers, &token));
        headers.remove("sec-fetch-site");
        headers.remove(header::COOKIE);
        assert!(!browser_challenge_allowed(&headers, &token));
    }

    #[test]
    fn verification_form_escapes_untrusted_values() {
        let html = form("\"><script>bad()</script>", Some("<script>bad()</script>"));
        assert!(!html.contains("<script>bad()</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
