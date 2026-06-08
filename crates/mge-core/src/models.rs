use std::fmt;
use std::str::FromStr;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::errors::MgeError;
use crate::markers::canonicalize_marker_value;

pub type CellId = u64;
pub type MarkerId = u32;
pub type PageId = u64;

macro_rules! enum_with_snake_names {
    ($ty:ident { $($variant:ident => $name:literal),+ $(,)? }) => {
        impl $ty {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $ty {
            type Err = MgeError;

            fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
                let normalized = canonicalize_marker_value(input);
                match normalized.as_str() {
                    $($name => Ok(Self::$variant),)+
                    _ => Err(MgeError::InvalidInput(format!(
                        "unknown {}: {}",
                        stringify!($ty),
                        input
                    ))),
                }
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    UserPreference,
    ProjectFact,
    TaskState,
    Decision,
    Procedure,
    ToolResult,
    Evidence,
    Hypothesis,
    TemporaryNote,
    DeprecatedFact,
}

enum_with_snake_names!(MemoryKind {
    UserPreference => "user_preference",
    ProjectFact => "project_fact",
    TaskState => "task_state",
    Decision => "decision",
    Procedure => "procedure",
    ToolResult => "tool_result",
    Evidence => "evidence",
    Hypothesis => "hypothesis",
    TemporaryNote => "temporary_note",
    DeprecatedFact => "deprecated_fact",
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Active,
    Temporary,
    Deprecated,
    Rejected,
    Superseded,
    Unverified,
    Verified,
}

enum_with_snake_names!(MemoryStatus {
    Active => "active",
    Temporary => "temporary",
    Deprecated => "deprecated",
    Rejected => "rejected",
    Superseded => "superseded",
    Unverified => "unverified",
    Verified => "verified",
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    UserConfirmed,
    AgentInferred,
    ToolObserved,
    ExternalUntrusted,
    SystemGenerated,
}

enum_with_snake_names!(TrustLevel {
    UserConfirmed => "user_confirmed",
    AgentInferred => "agent_inferred",
    ToolObserved => "tool_observed",
    ExternalUntrusted => "external_untrusted",
    SystemGenerated => "system_generated",
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityLevel {
    Public,
    Private,
    Confidential,
    SecretReference,
}

enum_with_snake_names!(SensitivityLevel {
    Public => "public",
    Private => "private",
    Confidential => "confidential",
    SecretReference => "secret_reference",
});

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallMode {
    Focused,
    Broad,
    FullScope,
}

enum_with_snake_names!(RecallMode {
    Focused => "focused",
    Broad => "broad",
    FullScope => "full_scope",
});

impl Default for RecallMode {
    fn default() -> Self {
        Self::Focused
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum MemoryValue {
    Text(String),
    Symbol(String),
    Number(f64),
    Boolean(bool),
    Timestamp(i64),
    Reference(String),
    Structured(serde_json::Value),
}

impl MemoryValue {
    pub fn to_plain_text(&self) -> String {
        match self {
            Self::Text(value) | Self::Symbol(value) | Self::Reference(value) => value.clone(),
            Self::Number(value) => value.to_string(),
            Self::Boolean(value) => value.to_string(),
            Self::Timestamp(value) => value.to_string(),
            Self::Structured(value) => value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MemorySource {
    pub source_type: String,
    pub reference: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MemoryCell {
    pub id: CellId,
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub value: MemoryValue,
    pub scope: String,
    pub status: MemoryStatus,
    pub trust: TrustLevel,
    pub sensitivity: SensitivityLevel,
    pub created_at: i64,
    pub updated_at: i64,
    pub markers: Vec<MarkerId>,
    pub source: Option<MemorySource>,
    pub links: Vec<CellId>,
}

impl MemoryCell {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CellId,
        kind: MemoryKind,
        subject: Option<String>,
        value: MemoryValue,
        scope: String,
        status: MemoryStatus,
        trust: TrustLevel,
        sensitivity: SensitivityLevel,
        markers: Vec<MarkerId>,
        source: Option<MemorySource>,
        links: Vec<CellId>,
    ) -> Self {
        let now = current_timestamp();
        Self {
            id,
            kind,
            subject,
            value,
            scope,
            status,
            trust,
            sensitivity,
            created_at: now,
            updated_at: now,
            markers,
            source,
            links,
        }
    }
}

pub fn current_timestamp() -> i64 {
    Utc::now().timestamp()
}
