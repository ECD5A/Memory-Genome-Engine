use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::models::{MemoryCell, MemoryStatus, SensitivityLevel};

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
        if !self.include_deprecated && cell.status == MemoryStatus::Deprecated {
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
