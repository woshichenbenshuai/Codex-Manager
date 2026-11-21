use base64::{engine::general_purpose::STANDARD, Engine};
use crypto_box::SecretKey;
use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use sha2::{Digest, Sha256};

pub(super) fn random_hex(bytes: usize) -> String {
    let mut data = vec![0; bytes];
    rand::rngs::OsRng.fill_bytes(&mut data);
    data.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn hash(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) fn generate_secret() -> String {
    let mut data = [0; 20];
    rand::rngs::OsRng.fill_bytes(&mut data);
    BASE32_NOPAD.encode(&data)
}

// The persisted RPC token is already shared by service/web and protected as a
// deployment secret. Domain separation keeps its authentication uses independent.
fn key() -> SecretKey {
    let bytes: [u8; 32] = Sha256::digest(format!(
        "codexmanager:2fa:encryption:v1:{}",
        super::rpc::rpc_auth_token()
    ))
    .into();
    SecretKey::from(bytes)
}

pub(super) fn seal(value: &str) -> Result<String, String> {
    key()
        .public_key()
        .seal(&mut rand::rngs::OsRng, value.as_bytes())
        .map(|bytes| STANDARD.encode(bytes))
        .map_err(|_| "2FA 数据加密失败".into())
}

pub(super) fn unseal(value: &str) -> Result<String, String> {
    let bytes = STANDARD.decode(value).map_err(|_| "2FA 数据格式错误")?;
    let plain = key()
        .unseal(&bytes)
        .map_err(|_| "2FA 数据解密失败，请恢复原来的 RPC token 文件")?;
    String::from_utf8(plain).map_err(|_| "2FA 数据格式错误".into())
}

pub(super) fn code(secret: &str, step: i64) -> Result<String, String> {
    let key = BASE32_NOPAD
        .decode(secret.as_bytes())
        .map_err(|_| "验证器密钥无效")?;
    if key.len() != 20 {
        return Err("验证器密钥长度无效".into());
    }
    let mut mac = Hmac::<Sha1>::new_from_slice(&key).map_err(|_| "验证器密钥无效")?;
    mac.update(&(step as u64).to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 15) as usize;
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    Ok(format!("{:06}", value % 1_000_000))
}

pub(super) fn verify(
    secret: &str,
    value: &str,
    now: i64,
    last_step: Option<i64>,
) -> Result<i64, String> {
    let value = value.trim();
    if value.len() != 6 || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err("动态验证码错误或已使用".into());
    }
    for offset in [0, -1, 1] {
        let step = now / 30 + offset;
        if step < 0 || last_step.is_some_and(|last| step <= last) {
            continue;
        }
        if super::rpc::constant_time_eq(value.as_bytes(), code(secret, step)?.as_bytes()) {
            return Ok(step);
        }
    }
    Err("动态验证码错误或已使用".into())
}

pub(super) fn receipt(mode: &str, primary_token: &str, generation: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(super::rpc::rpc_auth_token().as_bytes()).unwrap();
    mac.update(
        format!("codexmanager:2fa:receipt:v1:{mode}:{generation}:{primary_token}").as_bytes(),
    );
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc6238_sha1_and_replay_window() {
        let secret = BASE32_NOPAD.encode(b"12345678901234567890");
        assert_eq!(code(&secret, 1).unwrap(), "287082");
        assert_eq!(verify(&secret, "287082", 59, None).unwrap(), 1);
        assert!(verify(&secret, "287082", 59, Some(1)).is_err());
        assert!(verify(&secret, "287082", 150, None).is_err());
        assert!(verify(&secret, "28708x", 59, None).is_err());
    }
    #[test]
    fn sealed_values_and_receipts_are_bound_to_the_primary_login() {
        let cipher = seal("secret").unwrap();
        assert_ne!(cipher, "secret");
        assert_eq!(unseal(&cipher).unwrap(), "secret");
        assert_ne!(receipt("password", "a", "g"), receipt("password", "b", "g"));
        assert_ne!(receipt("password", "a", "g"), receipt("accounts", "a", "g"));
        assert_ne!(
            receipt("password", "a", "g"),
            receipt("password", "a", "next")
        );
    }
}
