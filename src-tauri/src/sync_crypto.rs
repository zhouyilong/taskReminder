//! 云同步端到端加密：上传前用同步密码加密数据库快照，下载后解密。
//!
//! 文件格式（所有整数为小端）：
//!
//! ```text
//! magic    8 字节  "TRENC\0\0\x01"
//! kdf      1 字节  1 = Argon2id
//! m_cost   4 字节  内存（KiB）
//! t_cost   4 字节  迭代次数
//! p_cost   4 字节  并行度
//! salt    16 字节
//! nonce   12 字节
//! 密文 + 16 字节认证标签（AES-256-GCM）
//! ```
//!
//! 头部整体作为 AEAD 附加数据参与认证，篡改参数或截断都会导致解密失败。
//! 每次加密使用新的随机盐与 nonce；KDF 参数写在头部，解密时按头部参数派生密钥。

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};

const MAGIC: &[u8; 8] = b"TRENC\0\0\x01";
const KDF_ARGON2ID: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const HEADER_LEN: usize = MAGIC.len() + 1 + 4 * 3 + SALT_LEN + NONCE_LEN;
const TAG_LEN: usize = 16;

/// 同步密码的最少字符数。
pub const MIN_PASSPHRASE_CHARS: usize = 8;

/// Argon2id 参数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    /// 默认参数：64 MiB、3 次迭代，桌面端派生一次约零点几秒。
    pub const DEFAULT: KdfParams = KdfParams {
        m_cost: 64 * 1024,
        t_cost: 3,
        p_cost: 1,
    };

    /// 解密时接受的参数上限，防止被篡改的文件让派生耗尽内存或时间。
    fn is_reasonable(&self) -> bool {
        (8..=1024 * 1024).contains(&self.m_cost)
            && (1..=20).contains(&self.t_cost)
            && (1..=16).contains(&self.p_cost)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CryptoError {
    /// 不是加密的同步文件。
    InvalidFormat,
    /// 密码错误，或文件被篡改、损坏。
    Decrypt,
    /// 同步密码为空或太短。
    WeakPassphrase,
    Internal(String),
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::InvalidFormat => write!(f, "远端文件不是有效的加密同步数据"),
            CryptoError::Decrypt => write!(f, "同步密码错误，或远端加密数据已损坏"),
            CryptoError::WeakPassphrase => {
                write!(f, "同步密码至少需要 {} 个字符", MIN_PASSPHRASE_CHARS)
            }
            CryptoError::Internal(message) => write!(f, "加密失败：{}", message),
        }
    }
}

pub fn validate_passphrase(passphrase: &str) -> Result<(), CryptoError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(CryptoError::WeakPassphrase);
    }
    Ok(())
}

/// 是否为本模块生成的加密文件。
pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

fn derive_key(passphrase: &str, salt: &[u8], params: KdfParams) -> Result<[u8; 32], CryptoError> {
    let argon_params = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(32))
        .map_err(|e| CryptoError::Internal(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut key = [0u8; 32];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| CryptoError::Internal(e.to_string()))?;
    Ok(key)
}

pub fn encrypt(plain: &[u8], passphrase: &str, params: KdfParams) -> Result<Vec<u8>, CryptoError> {
    validate_passphrase(passphrase)?;
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(MAGIC);
    header.push(KDF_ARGON2ID);
    header.extend_from_slice(&params.m_cost.to_le_bytes());
    header.extend_from_slice(&params.t_cost.to_le_bytes());
    header.extend_from_slice(&params.p_cost.to_le_bytes());
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);

    let key = derive_key(passphrase, &salt, params)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| CryptoError::Internal(e.to_string()))?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plain,
                aad: &header,
            },
        )
        .map_err(|e| CryptoError::Internal(e.to_string()))?;

    let mut out = header;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt(data: &[u8], passphrase: &str) -> Result<Vec<u8>, CryptoError> {
    if !is_encrypted(data) || data.len() < HEADER_LEN + TAG_LEN {
        return Err(CryptoError::InvalidFormat);
    }
    let mut offset = MAGIC.len();
    if data[offset] != KDF_ARGON2ID {
        return Err(CryptoError::InvalidFormat);
    }
    offset += 1;
    let read_u32 =
        |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    let params = KdfParams {
        m_cost: read_u32(offset),
        t_cost: read_u32(offset + 4),
        p_cost: read_u32(offset + 8),
    };
    offset += 12;
    if !params.is_reasonable() {
        return Err(CryptoError::InvalidFormat);
    }
    let salt = &data[offset..offset + SALT_LEN];
    offset += SALT_LEN;
    let nonce = &data[offset..offset + NONCE_LEN];
    offset += NONCE_LEN;
    let (header, ciphertext) = data.split_at(offset);

    let key = derive_key(passphrase, salt, params)?;
    let cipher =
        Aes256Gcm::new_from_slice(&key).map_err(|e| CryptoError::Internal(e.to_string()))?;
    cipher
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: header,
            },
        )
        .map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
pub(crate) const TEST_PARAMS: KdfParams = KdfParams {
    m_cost: 64,
    t_cost: 1,
    p_cost: 1,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_fresh_salt_and_nonce() {
        let plain = b"SQLite format 3\0 some rows".to_vec();
        let first = encrypt(&plain, "correct horse", TEST_PARAMS).unwrap();
        let second = encrypt(&plain, "correct horse", TEST_PARAMS).unwrap();
        assert!(is_encrypted(&first));
        assert_ne!(first, second, "each upload uses a new salt and nonce");
        assert!(!first
            .windows(plain.len())
            .any(|window| window == plain.as_slice()));
        assert_eq!(decrypt(&first, "correct horse").unwrap(), plain);
        assert_eq!(decrypt(&second, "correct horse").unwrap(), plain);
    }

    #[test]
    fn wrong_passphrase_or_tampering_fails() {
        let data = encrypt(b"secret tasks", "correct horse", TEST_PARAMS).unwrap();
        assert_eq!(decrypt(&data, "wrong horse!"), Err(CryptoError::Decrypt));

        // 篡改头部参数（参与认证）或密文都会失败。
        let mut tampered = data.clone();
        tampered[MAGIC.len() + 5] ^= 1; // t_cost
        assert!(decrypt(&tampered, "correct horse").is_err());
        let mut tampered = data.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert_eq!(
            decrypt(&tampered, "correct horse"),
            Err(CryptoError::Decrypt)
        );
        // 截断。
        assert!(decrypt(&data[..HEADER_LEN + 4], "correct horse").is_err());
    }

    #[test]
    fn rejects_plain_files_and_unreasonable_params() {
        assert_eq!(
            decrypt(b"SQLite format 3\0", "correct horse"),
            Err(CryptoError::InvalidFormat)
        );
        let mut data = encrypt(b"x", "correct horse", TEST_PARAMS).unwrap();
        let m_cost_at = MAGIC.len() + 1;
        data[m_cost_at..m_cost_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decrypt(&data, "correct horse"),
            Err(CryptoError::InvalidFormat)
        );
    }

    #[test]
    fn passphrase_must_be_long_enough() {
        assert_eq!(
            encrypt(b"x", "short", TEST_PARAMS),
            Err(CryptoError::WeakPassphrase)
        );
        assert!(validate_passphrase("八个字的中文密码").is_ok());
        assert!(validate_passphrase("1234567").is_err());
    }
}
