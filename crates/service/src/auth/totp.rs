use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use crypto_box::SecretKey;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;

const TOTP_STEP_SECONDS: i64 = 30;
const TOTP_DIGITS: u32 = 6;
const TOTP_SECRET_BYTES: usize = 20;

pub(crate) const TOTP_ENCRYPTION_KEY_ENV: &str = "CODEXMANAGER_WEB_TOTP_ENCRYPTION_KEY";

pub(crate) fn generate_secret() -> String {
    let mut bytes = [0u8; TOTP_SECRET_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    base32_encode(&bytes)
}

pub(crate) fn otpauth_uri(username: &str, secret: &str) -> String {
    format!(
        "otpauth://totp/CodexManager:{}?secret={}&issuer=CodexManager&algorithm=SHA1&digits=6&period=30",
        percent_encode_label(username),
        secret,
    )
}

pub(crate) fn verify_code(
    secret: &str,
    code: &str,
    now: i64,
    last_used_step: Option<i64>,
) -> Result<i64, String> {
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("验证码错误".to_string());
    }
    let current_step = now.div_euclid(TOTP_STEP_SECONDS);
    for offset in [0_i64, -1, 1] {
        let step = current_step.saturating_add(offset);
        if last_used_step.is_some_and(|last| step <= last) {
            continue;
        }
        let expected = hotp(secret, step)?;
        if constant_time_eq(code.as_bytes(), expected.as_bytes()) {
            return Ok(step);
        }
    }
    Err("验证码错误".to_string())
}

pub(crate) fn encrypt_secret(secret: &str) -> Result<String, String> {
    let key = encryption_key()?;
    let ciphertext = key
        .public_key()
        .seal(&mut rand::rngs::OsRng, secret.as_bytes())
        .map_err(|_| "验证器密钥加密失败".to_string())?;
    Ok(BASE64_STANDARD.encode(ciphertext))
}

pub(crate) fn decrypt_secret(ciphertext: &str) -> Result<String, String> {
    let key = encryption_key()?;
    let bytes = BASE64_STANDARD
        .decode(ciphertext)
        .map_err(|_| "验证器密钥格式无效".to_string())?;
    let plaintext = key
        .unseal(&bytes)
        .map_err(|_| "验证器密钥解密失败，请检查 TOTP 加密密钥".to_string())?;
    let secret = String::from_utf8(plaintext).map_err(|_| "验证器密钥编码无效".to_string())?;
    let decoded = base32_decode(&secret)?;
    if decoded.len() != TOTP_SECRET_BYTES {
        return Err("验证器密钥长度无效".to_string());
    }
    Ok(secret)
}

pub(crate) fn validate_encryption_key() -> Result<(), String> {
    let _ = encryption_key()?;
    Ok(())
}

fn encryption_key() -> Result<SecretKey, String> {
    let raw = std::env::var(TOTP_ENCRYPTION_KEY_ENV).map_err(|_| {
        format!("缺少 {TOTP_ENCRYPTION_KEY_ENV}，无法启用账号验证器；请配置稳定的 32 字节密钥")
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{TOTP_ENCRYPTION_KEY_ENV} 不能为空"));
    }
    let bytes = if trimmed.len() == 64 && trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut bytes = [0u8; 32];
        for (index, chunk) in trimmed.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("00"), 16)
                .map_err(|_| format!("{TOTP_ENCRYPTION_KEY_ENV} 不是有效密钥"))?;
        }
        bytes
    } else {
        let decoded = BASE64_STANDARD.decode(trimmed).map_err(|_| {
            format!("{TOTP_ENCRYPTION_KEY_ENV} 必须是 32 字节 Base64 或 64 位十六进制")
        })?;
        if decoded.len() != 32 {
            return Err(format!("{TOTP_ENCRYPTION_KEY_ENV} 必须正好是 32 字节"));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        bytes
    };
    Ok(SecretKey::from(bytes))
}

fn hotp(secret: &str, counter: i64) -> Result<String, String> {
    let key = base32_decode(secret)?;
    if key.len() != TOTP_SECRET_BYTES {
        return Err("验证器密钥长度无效".to_string());
    }
    let mut message = [0u8; 8];
    message.copy_from_slice(&(counter.max(0) as u64).to_be_bytes());
    let digest = hmac_sha1(&key, &message);
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    Ok(format!("{:06}", binary % 10_u32.pow(TOTP_DIGITS)))
}

fn base32_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut output = String::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for byte in bytes {
        buffer = (buffer << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            output.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        output.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    output
}

fn base32_decode(input: &str) -> Result<Vec<u8>, String> {
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut output = Vec::new();
    for byte in input.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        let upper = byte.to_ascii_uppercase();
        let value = match upper {
            b'A'..=b'Z' => upper - b'A',
            b'2'..=b'7' => upper - b'2' + 26,
            b'=' => break,
            _ => return Err("无效的 Base32 验证器密钥".to_string()),
        };
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Ok(output)
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC-SHA1 accepts keys of any length");
    mac.update(message);
    let digest = mac.finalize().into_bytes();
    let mut output = [0u8; 20];
    output.copy_from_slice(&digest);
    output
}

fn percent_encode_label(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::{base32_encode, hotp, verify_code};

    #[test]
    fn rfc4226_hotp_sha1_vector_matches() {
        let secret = base32_encode(b"12345678901234567890");
        assert_eq!(hotp(&secret, 0).unwrap(), "755224");
        assert_eq!(hotp(&secret, 1).unwrap(), "287082");
    }

    #[test]
    fn rfc6238_sha1_time_vectors_match() {
        let secret = base32_encode(b"12345678901234567890");
        assert_eq!(hotp(&secret, 59 / 30).unwrap(), "287082");
        assert_eq!(hotp(&secret, 1_111_111_109 / 30).unwrap(), "081804");
        assert_eq!(hotp(&secret, 1_234_567_890 / 30).unwrap(), "005924");
        assert_eq!(hotp(&secret, 2_000_000_000 / 30).unwrap(), "279037");
    }

    #[test]
    fn rejects_reused_step() {
        let secret = base32_encode(b"12345678901234567890");
        let code = hotp(&secret, 1).unwrap();
        assert!(verify_code(&secret, &code, 30, Some(1)).is_err());
        assert_eq!(verify_code(&secret, &code, 30, None).unwrap(), 1);
    }

    #[test]
    fn accepts_one_clock_window_but_rejects_older_codes() {
        let secret = base32_encode(b"12345678901234567890");
        let code = hotp(&secret, 1).unwrap();
        assert_eq!(verify_code(&secret, &code, 60, None).unwrap(), 1);
        assert!(verify_code(&secret, &code, 90, None).is_err());
    }

    #[test]
    fn rejects_malformed_codes() {
        let secret = base32_encode(b"12345678901234567890");
        assert!(verify_code(&secret, "12345", 0, None).is_err());
        assert!(verify_code(&secret, "12a456", 0, None).is_err());
    }

    #[test]
    fn rejects_invalid_or_wrong_length_secrets() {
        assert!(hotp("not-base32!", 1).is_err());
        assert!(hotp("JBSWY3DP", 1).is_err());
    }
}
