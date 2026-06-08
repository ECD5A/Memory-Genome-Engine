use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::str::FromStr;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::errors::MgeError;
use crate::markers::canonicalize_marker_value;

pub type CellId = u64;
pub type MarkerId = u32;
pub type PageId = u64;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct MarkerGenome {
    pub scope: Option<MarkerId>,
    pub kind: Option<MarkerId>,
    pub status: Option<MarkerId>,
    pub trust: Option<MarkerId>,
    pub sensitivity: Option<MarkerId>,
    #[serde(default)]
    pub subject: Vec<MarkerId>,
    #[serde(default)]
    pub value_domain: Vec<MarkerId>,
    #[serde(default)]
    pub custom: Vec<MarkerId>,
}

impl MarkerGenome {
    pub fn from_flattened(marker_ids: impl IntoIterator<Item = MarkerId>) -> Self {
        Self {
            custom: sorted_unique(marker_ids),
            ..Self::default()
        }
    }

    pub fn from_canonical_markers(
        markers: impl IntoIterator<Item = (String, MarkerId)>,
        explicit_markers: &[String],
    ) -> Self {
        let explicit = explicit_markers
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut genome = Self::default();

        for (marker, marker_id) in markers {
            if explicit.contains(marker.as_str()) {
                push_unique_marker(&mut genome.custom, marker_id);
                continue;
            }

            match marker.split_once(':').map(|(category, _)| category) {
                Some("scope") => genome.scope = Some(marker_id),
                Some("kind") => genome.kind = Some(marker_id),
                Some("status") => genome.status = Some(marker_id),
                Some("trust") => genome.trust = Some(marker_id),
                Some("sensitivity") => genome.sensitivity = Some(marker_id),
                Some("subject") => push_unique_marker(&mut genome.subject, marker_id),
                Some("value") => push_unique_marker(&mut genome.value_domain, marker_id),
                _ => push_unique_marker(&mut genome.value_domain, marker_id),
            }
        }

        genome.normalize();
        genome
    }

    pub fn is_empty(&self) -> bool {
        self.scope.is_none()
            && self.kind.is_none()
            && self.status.is_none()
            && self.trust.is_none()
            && self.sensitivity.is_none()
            && self.subject.is_empty()
            && self.value_domain.is_empty()
            && self.custom.is_empty()
    }

    pub fn all_marker_ids(&self) -> Vec<MarkerId> {
        let mut markers = self.system_marker_ids();
        markers.extend(self.custom.iter().copied());
        sorted_unique(markers)
    }

    pub fn iter_all_marker_ids(&self) -> impl Iterator<Item = MarkerId> + '_ {
        self.iter_system_marker_ids()
            .chain(self.custom.iter().copied())
    }

    pub fn system_marker_ids(&self) -> Vec<MarkerId> {
        let mut markers = Vec::new();
        markers.extend(self.scope);
        markers.extend(self.kind);
        markers.extend(self.status);
        markers.extend(self.trust);
        markers.extend(self.sensitivity);
        markers.extend(self.subject.iter().copied());
        markers.extend(self.value_domain.iter().copied());
        sorted_unique(markers)
    }

    pub fn iter_system_marker_ids(&self) -> impl Iterator<Item = MarkerId> + '_ {
        [
            self.scope,
            self.kind,
            self.status,
            self.trust,
            self.sensitivity,
        ]
        .into_iter()
        .flatten()
        .chain(self.subject.iter().copied())
        .chain(self.value_domain.iter().copied())
    }

    pub fn custom_marker_ids(&self) -> Vec<MarkerId> {
        sorted_unique(self.custom.iter().copied())
    }

    pub fn iter_custom_marker_ids(&self) -> impl Iterator<Item = MarkerId> + '_ {
        self.custom.iter().copied()
    }

    pub fn scope_marker(&self) -> Option<MarkerId> {
        self.scope
    }

    pub fn scope_marker_id(&self) -> Option<MarkerId> {
        self.scope
    }

    pub fn kind_marker(&self) -> Option<MarkerId> {
        self.kind
    }

    pub fn kind_marker_id(&self) -> Option<MarkerId> {
        self.kind
    }

    pub fn status_marker(&self) -> Option<MarkerId> {
        self.status
    }

    pub fn status_marker_id(&self) -> Option<MarkerId> {
        self.status
    }

    pub fn trust_marker(&self) -> Option<MarkerId> {
        self.trust
    }

    pub fn trust_marker_id(&self) -> Option<MarkerId> {
        self.trust
    }

    pub fn sensitivity_marker(&self) -> Option<MarkerId> {
        self.sensitivity
    }

    pub fn sensitivity_marker_id(&self) -> Option<MarkerId> {
        self.sensitivity
    }

    pub fn contains_marker(&self, marker_id: MarkerId) -> bool {
        self.scope == Some(marker_id)
            || self.kind == Some(marker_id)
            || self.status == Some(marker_id)
            || self.trust == Some(marker_id)
            || self.sensitivity == Some(marker_id)
            || self.subject.contains(&marker_id)
            || self.value_domain.contains(&marker_id)
            || self.custom.contains(&marker_id)
    }

    pub fn marker_summary(cells: &[MemoryCell]) -> Vec<MarkerId> {
        let mut summary = BTreeSet::new();
        for cell in cells {
            cell.for_each_marker_id_for_indexing(|marker_id| {
                summary.insert(marker_id);
            });
        }
        summary.into_iter().collect()
    }

    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        for marker_id in self.all_marker_ids() {
            hasher.update(marker_id.to_le_bytes());
        }
        hex_lower(&hasher.finalize())
    }

    fn normalize(&mut self) {
        self.subject = sorted_unique(self.subject.iter().copied());
        self.value_domain = sorted_unique(self.value_domain.iter().copied());
        self.custom = sorted_unique(self.custom.iter().copied());
    }
}

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
    #[serde(default)]
    pub marker_genome: MarkerGenome,
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
            marker_genome: MarkerGenome::from_flattened(markers.iter().copied()),
            markers,
            source,
            links,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_marker_genome(
        id: CellId,
        kind: MemoryKind,
        subject: Option<String>,
        value: MemoryValue,
        scope: String,
        status: MemoryStatus,
        trust: TrustLevel,
        sensitivity: SensitivityLevel,
        marker_genome: MarkerGenome,
        source: Option<MemorySource>,
        links: Vec<CellId>,
    ) -> Self {
        let now = current_timestamp();
        let markers = marker_genome.all_marker_ids();
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
            marker_genome,
            source,
            links,
        }
    }

    pub fn marker_ids_for_indexing(&self) -> Vec<MarkerId> {
        let mut markers = BTreeSet::new();
        self.for_each_marker_id_for_indexing(|marker_id| {
            markers.insert(marker_id);
        });
        markers.into_iter().collect()
    }

    pub fn flattened_marker_ids(&self) -> &[MarkerId] {
        &self.markers
    }

    pub fn iter_flattened_marker_ids(&self) -> impl Iterator<Item = MarkerId> + '_ {
        self.markers.iter().copied()
    }

    pub fn for_each_marker_id_for_indexing(&self, mut visit: impl FnMut(MarkerId)) {
        if !self.markers.is_empty() {
            for marker_id in &self.markers {
                visit(*marker_id);
            }
            return;
        }

        for marker_id in self.marker_genome.iter_all_marker_ids() {
            visit(marker_id);
        }
    }

    pub fn marker_overlap_count(&self, query_marker_set: &HashSet<MarkerId>) -> usize {
        query_marker_set
            .iter()
            .filter(|marker_id| self.contains_marker(**marker_id))
            .count()
    }

    pub fn contains_marker(&self, marker_id: MarkerId) -> bool {
        self.markers.contains(&marker_id)
            || (!self.marker_genome.is_empty() && self.marker_genome.contains_marker(marker_id))
    }
}

pub fn current_timestamp() -> i64 {
    Utc::now().timestamp()
}

fn sorted_unique(marker_ids: impl IntoIterator<Item = MarkerId>) -> Vec<MarkerId> {
    marker_ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn push_unique_marker(markers: &mut Vec<MarkerId>, marker_id: MarkerId) {
    if !markers.contains(&marker_id) {
        markers.push(marker_id);
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
