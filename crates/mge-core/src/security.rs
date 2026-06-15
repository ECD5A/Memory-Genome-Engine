use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::errors::{MgeError, Result};
use crate::models::{MemoryCell, MemoryStatus, SensitivityLevel};

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
    pub session_unlock_required: bool,
    pub metadata_plaintext: bool,
    pub implementation_status: String,
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
