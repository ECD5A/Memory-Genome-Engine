use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::errors::{MgeError, Result};
use crate::models::{MemoryCell, MemoryStatus, SensitivityLevel};

pub const ENCRYPTION_ALGORITHM: &str = "xchacha20poly1305";
pub const ENCRYPTION_VERSION: u32 = 1;
pub const KDF_ALGORITHM: &str = "argon2id";
pub const KDF_VERSION: u32 = 1;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const SALT_LEN: usize = 16;
const KDF_MEMORY_COST_KIB: u32 = 19_456;
const KDF_TIME_COST: u32 = 2;
const KDF_PARALLELISM: u32 = 1;
const KEY_CHECK_PLAINTEXT: &[u8] = b"MGE key check v1";
const KEY_CHECK_AAD: &[u8] = b"mge:key_check:v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityMode {
    Unencrypted,
    Encrypted,
}

impl Default for SecurityMode {
    fn default() -> Self {
        Self::Unencrypted
    }
}

impl SecurityMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unencrypted => "unencrypted",
            Self::Encrypted => "encrypted",
        }
    }

    pub fn is_encrypted(&self) -> bool {
        *self == Self::Encrypted
    }
}

impl fmt::Display for SecurityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SecurityMode {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "unencrypted" | "none" | "plaintext" => Ok(Self::Unencrypted),
            "encrypted" => Ok(Self::Encrypted),
            other => Err(MgeError::InvalidInput(format!(
                "unknown security mode: {other}; supported: unencrypted, encrypted"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecurityConfig {
    pub mode: SecurityMode,
    pub payload_encryption: bool,
    pub hot_payload_encryption: bool,
    pub sealed_page_payload_encryption: bool,
    pub session_unlock_required: bool,
    pub key_verification_configured: bool,
    pub metadata_plaintext: bool,
    pub implementation_status: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SecurityMetadata {
    #[serde(default)]
    pub encryption: Option<EncryptionMetadata>,
    #[serde(default)]
    pub kdf: Option<KdfMetadata>,
    #[serde(default)]
    pub key_check: Option<EncryptedPayload>,
}

impl SecurityMetadata {
    pub fn is_configured(&self) -> bool {
        self.encryption.is_some() && self.kdf.is_some() && self.key_check.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncryptionMetadata {
    pub algorithm: String,
    pub version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KdfMetadata {
    pub algorithm: String,
    pub version: u32,
    pub salt: Vec<u8>,
    pub memory_cost_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
    pub output_len: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EncryptedPayload {
    pub version: u32,
    pub algorithm: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub struct SessionKey {
    bytes: [u8; KEY_LEN],
}

impl SessionKey {
    fn new(bytes: [u8; KEY_LEN]) -> Self {
        Self { bytes }
    }

    fn expose(&self) -> &[u8; KEY_LEN] {
        &self.bytes
    }
}

impl Drop for SessionKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionKey(<redacted>)")
    }
}

pub fn create_security_metadata(passphrase: &str) -> Result<(SecurityMetadata, SessionKey)> {
    if passphrase.is_empty() {
        return Err(MgeError::InvalidInput(
            "passphrase env value must not be empty".to_string(),
        ));
    }

    let mut salt = vec![0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let kdf = KdfMetadata {
        algorithm: KDF_ALGORITHM.to_string(),
        version: KDF_VERSION,
        salt,
        memory_cost_kib: KDF_MEMORY_COST_KIB,
        time_cost: KDF_TIME_COST,
        parallelism: KDF_PARALLELISM,
        output_len: KEY_LEN as u32,
    };
    let key = derive_session_key(&kdf, passphrase)?;
    let key_check = encrypt_payload(&key, KEY_CHECK_AAD, KEY_CHECK_PLAINTEXT)?;
    Ok((
        SecurityMetadata {
            encryption: Some(EncryptionMetadata {
                algorithm: ENCRYPTION_ALGORITHM.to_string(),
                version: ENCRYPTION_VERSION,
            }),
            kdf: Some(kdf),
            key_check: Some(key_check),
        },
        key,
    ))
}

pub fn unlock_security_metadata(
    metadata: &SecurityMetadata,
    passphrase: &str,
) -> Result<SessionKey> {
    if passphrase.is_empty() {
        return Err(MgeError::InvalidInput(
            "passphrase env value must not be empty".to_string(),
        ));
    }
    let encryption = metadata.encryption.as_ref().ok_or_else(|| {
        MgeError::StoreLocked(
            "encrypted store has no encryption metadata; initialize with --passphrase-env"
                .to_string(),
        )
    })?;
    if encryption.algorithm != ENCRYPTION_ALGORITHM || encryption.version != ENCRYPTION_VERSION {
        return Err(MgeError::StoreLocked(format!(
            "unsupported encrypted store scheme {} v{}",
            encryption.algorithm, encryption.version
        )));
    }
    let kdf = metadata.kdf.as_ref().ok_or_else(|| {
        MgeError::StoreLocked(
            "encrypted store has no KDF metadata; initialize with --passphrase-env".to_string(),
        )
    })?;
    let key = derive_session_key(kdf, passphrase)?;
    let key_check = metadata.key_check.as_ref().ok_or_else(|| {
        MgeError::StoreLocked(
            "encrypted store has no key verification metadata; initialize with --passphrase-env"
                .to_string(),
        )
    })?;
    let plaintext = decrypt_payload(&key, KEY_CHECK_AAD, key_check)?;
    if plaintext != KEY_CHECK_PLAINTEXT {
        return Err(MgeError::AuthenticationFailed(
            "passphrase could not unlock this store".to_string(),
        ));
    }
    Ok(key)
}

pub fn encrypt_payload(key: &SessionKey, aad: &[u8], plaintext: &[u8]) -> Result<EncryptedPayload> {
    let mut nonce = vec![0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
        .map_err(|_| MgeError::Crypto("failed to initialize AEAD".to_string()))?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| MgeError::Crypto("payload encryption failed".to_string()))?;
    Ok(EncryptedPayload {
        version: ENCRYPTION_VERSION,
        algorithm: ENCRYPTION_ALGORITHM.to_string(),
        nonce,
        ciphertext,
    })
}

pub fn decrypt_payload(
    key: &SessionKey,
    aad: &[u8],
    encrypted: &EncryptedPayload,
) -> Result<Vec<u8>> {
    if encrypted.version != ENCRYPTION_VERSION || encrypted.algorithm != ENCRYPTION_ALGORITHM {
        return Err(MgeError::StorageFormat(format!(
            "unsupported encrypted payload scheme {} v{}",
            encrypted.algorithm, encrypted.version
        )));
    }
    if encrypted.nonce.len() != NONCE_LEN {
        return Err(MgeError::StorageFormat(format!(
            "encrypted payload nonce must be {NONCE_LEN} bytes"
        )));
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key.expose())
        .map_err(|_| MgeError::Crypto("failed to initialize AEAD".to_string()))?;
    cipher
        .decrypt(
            XNonce::from_slice(&encrypted.nonce),
            Payload {
                msg: encrypted.ciphertext.as_ref(),
                aad,
            },
        )
        .map_err(|_| {
            MgeError::AuthenticationFailed("encrypted payload authentication failed".to_string())
        })
}

fn derive_session_key(kdf: &KdfMetadata, passphrase: &str) -> Result<SessionKey> {
    if kdf.algorithm != KDF_ALGORITHM || kdf.version != KDF_VERSION {
        return Err(MgeError::StoreLocked(format!(
            "unsupported KDF {} v{}",
            kdf.algorithm, kdf.version
        )));
    }
    if kdf.output_len != KEY_LEN as u32 {
        return Err(MgeError::StoreLocked(format!(
            "unsupported KDF output length {}",
            kdf.output_len
        )));
    }
    let params = Params::new(
        kdf.memory_cost_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|err| MgeError::Crypto(format!("invalid Argon2 parameters: {err}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, &mut key)
        .map_err(|err| MgeError::Crypto(format!("key derivation failed: {err}")))?;
    Ok(SessionKey::new(key))
}

pub trait SecurityProvider {
    fn seal_page_bytes(&self, page_bytes: &[u8]) -> Result<Vec<u8>>;
    fn open_page_bytes(&self, stored_bytes: &[u8]) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoSecurity;

impl SecurityProvider for NoSecurity {
    fn seal_page_bytes(&self, page_bytes: &[u8]) -> Result<Vec<u8>> {
        // Future pipeline hook: page bytes -> encryption/session policy -> stored bytes.
        // v0.1 deliberately stores plaintext bytes and does not pretend to encrypt.
        Ok(page_bytes.to_vec())
    }

    fn open_page_bytes(&self, stored_bytes: &[u8]) -> Result<Vec<u8>> {
        // Future pipeline hook: stored bytes -> authenticated decrypt -> page bytes.
        Ok(stored_bytes.to_vec())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    IncludeDeprecated,
    IncludeRejected,
    ReadSecretReferences,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentCapabilities {
    pub capabilities: BTreeSet<AgentCapability>,
}

impl AgentCapabilities {
    pub fn new(capabilities: impl IntoIterator<Item = AgentCapability>) -> Self {
        Self {
            capabilities: capabilities.into_iter().collect(),
        }
    }

    pub fn contains(&self, capability: AgentCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecallPolicy {
    pub include_deprecated: bool,
    pub include_rejected: bool,
    pub allow_secret_references: bool,
}

impl Default for RecallPolicy {
    fn default() -> Self {
        Self {
            include_deprecated: false,
            include_rejected: false,
            allow_secret_references: false,
        }
    }
}

impl RecallPolicy {
    pub fn from_capabilities(capabilities: &AgentCapabilities) -> Self {
        Self {
            include_deprecated: capabilities.contains(AgentCapability::IncludeDeprecated),
            include_rejected: capabilities.contains(AgentCapability::IncludeRejected),
            allow_secret_references: capabilities.contains(AgentCapability::ReadSecretReferences),
        }
    }

    pub fn permits_cell(&self, cell: &MemoryCell) -> bool {
        if !self.include_deprecated
            && matches!(
                cell.status,
                MemoryStatus::Deprecated | MemoryStatus::Superseded
            )
        {
            return false;
        }
        if !self.include_rejected && cell.status == MemoryStatus::Rejected {
            return false;
        }
        if !self.allow_secret_references && cell.sensitivity == SensitivityLevel::SecretReference {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuditEvent {
    pub event_type: String,
    pub summary: String,
}

pub trait AuditLogger {
    fn record(&self, event: &AuditEvent) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopAuditLogger;

impl AuditLogger for NoopAuditLogger {
    fn record(&self, _event: &AuditEvent) -> Result<()> {
        Ok(())
    }
}
