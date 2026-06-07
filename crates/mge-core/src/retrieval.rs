use std::collections::HashSet;

use crate::errors::Result;
use crate::markers::{canonicalize_marker_value, tokenize_keywords, MarkerDictionary};
use crate::models::{MemoryCell, MemoryKind, MemoryStatus, SensitivityLevel, TrustLevel};
use crate::packet::{ContextDebugInfo, ContextMemoryItem, ContextPacket, ContextScoreDebugItem};
use crate::security::{AgentCapabilities, RecallPolicy};

pub trait Retriever {
    fn recall(&self, request: RecallRequest) -> Result<ContextPacket>;
}

#[derive(Clone, Debug)]
pub struct RecallRequest {
    pub query: String,
    pub markers: Vec<String>,
    pub scope: Option<String>,
    pub kind: Option<MemoryKind>,
    pub max_items: usize,
    pub include_deprecated: bool,
    pub include_secret_references: bool,
    pub policy: RecallPolicy,
    pub capabilities: AgentCapabilities,
}

impl RecallRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            markers: Vec::new(),
            scope: None,
            kind: None,
            max_items: 5,
            include_deprecated: false,
            include_secret_references: false,
            policy: RecallPolicy::default(),
            capabilities: AgentCapabilities::default(),
        }
    }

    pub fn effective_policy(&self) -> RecallPolicy {
        let capability_policy = RecallPolicy::from_capabilities(&self.capabilities);
        RecallPolicy {
            include_deprecated: self.policy.include_deprecated
                || capability_policy.include_deprecated
                || self.include_deprecated,
            include_rejected: self.policy.include_rejected
                || capability_policy.include_rejected
                || self.include_deprecated,
            allow_secret_references: self.policy.allow_secret_references
                || capability_policy.allow_secret_references
                || self.include_secret_references,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RankedCell {
    pub cell: MemoryCell,
    pub score: i64,
    pub score_detail: ContextScoreDebugItem,
}

pub fn score_cell(
    cell: &MemoryCell,
    request: &RecallRequest,
    query_marker_ids: &[u32],
    query_tokens: &[String],
) -> Option<i64> {
    score_cell_debug(cell, request, query_marker_ids, query_tokens).map(|detail| detail.score)
}

pub fn score_cell_debug(
    cell: &MemoryCell,
    request: &RecallRequest,
    query_marker_ids: &[u32],
    query_tokens: &[String],
) -> Option<ContextScoreDebugItem> {
    if let Some(kind) = request.kind {
        if cell.kind != kind {
            return None;
        }
    }

    if let Some(scope) = &request.scope {
        if canonicalize_marker_value(&cell.scope) != canonicalize_marker_value(scope) {
            return None;
        }
    }

    if !request.effective_policy().permits_cell(cell) {
        return None;
    }

    let query_marker_set: HashSet<u32> = query_marker_ids.iter().copied().collect();
    let marker_overlap = cell
        .markers
        .iter()
        .filter(|marker| query_marker_set.contains(marker))
        .count() as i64;

    let query_token_set: HashSet<&str> = query_tokens.iter().map(String::as_str).collect();
    let value_tokens = tokenize_keywords(&cell.value.to_plain_text());
    let value_overlap = value_tokens
        .iter()
        .filter(|token| query_token_set.contains(token.as_str()))
        .count() as i64;
    let exact_value_match = exact_canonical_match(&cell.value.to_plain_text(), &request.query);

    let exact_subject_match = cell.subject.as_ref().is_some_and(|subject| {
        let subject_tokens = tokenize_keywords(subject);
        !subject_tokens.is_empty()
            && subject_tokens
                .iter()
                .all(|token| query_token_set.contains(token.as_str()))
    });

    let marker_overlap_score = marker_overlap * 10;
    let exact_subject_score = if exact_subject_match { 5 } else { 0 };
    let value_overlap_score = if value_overlap > 0 {
        value_overlap.min(3) * 3
    } else {
        0
    };
    let exact_value_score = if exact_value_match { 3 } else { 0 };

    let relevance =
        marker_overlap_score + exact_subject_score + value_overlap_score + exact_value_score;

    if relevance <= 0 {
        return None;
    }

    let trust_bonus = trust_bonus(cell.trust);
    let status_bonus = status_bonus(cell.status);
    let sensitivity_penalty = sensitivity_penalty(cell.sensitivity);
    let score = relevance + trust_bonus + status_bonus - sensitivity_penalty;

    Some(ContextScoreDebugItem {
        cell_id: cell.id,
        score,
        marker_overlap,
        marker_overlap_score,
        exact_subject_match,
        exact_subject_score,
        value_overlap,
        value_overlap_score,
        exact_value_match,
        exact_value_score,
        trust_bonus,
        status_bonus,
        sensitivity_penalty,
    })
}

pub fn build_context_packet(
    query: String,
    ranked: &[RankedCell],
    dictionary: &MarkerDictionary,
    debug: ContextDebugInfo,
    max_items: usize,
) -> ContextPacket {
    let relevant_memory = ranked
        .iter()
        .take(max_items)
        .map(|ranked| {
            let markers = ranked
                .cell
                .markers
                .iter()
                .filter_map(|marker| dictionary.marker(*marker).map(str::to_string))
                .collect();

            ContextMemoryItem {
                kind: ranked.cell.kind,
                content: ranked.cell.value.to_plain_text(),
                trust: ranked.cell.trust,
                status: ranked.cell.status,
                scope: ranked.cell.scope.clone(),
                sensitivity: ranked.cell.sensitivity,
                markers,
            }
        })
        .collect::<Vec<_>>();

    let score_details = ranked
        .iter()
        .take(max_items)
        .map(|ranked| ranked.score_detail.clone())
        .collect::<Vec<_>>();

    let includes_deprecated_or_rejected = relevant_memory.iter().any(|item| {
        matches!(
            item.status,
            MemoryStatus::Deprecated | MemoryStatus::Rejected
        )
    });
    let includes_secret_references = relevant_memory
        .iter()
        .any(|item| item.sensitivity == SensitivityLevel::SecretReference);

    let mut constraints = Vec::new();
    let mut warnings = Vec::new();
    if relevant_memory.is_empty() {
        warnings.push("No relevant memory matched the query.".to_string());
    }
    if includes_deprecated_or_rejected {
        warnings
            .push("Deprecated or rejected memories were included by explicit policy.".to_string());
    } else {
        constraints.push("Do not use deprecated or rejected memories.".to_string());
    }
    if includes_secret_references {
        warnings.push("SecretReference cells were included by explicit policy.".to_string());
    } else {
        constraints.push("Do not expose secret_reference cells.".to_string());
    }

    ContextPacket {
        query,
        relevant_memory,
        constraints,
        warnings,
        debug: ContextDebugInfo {
            score_details,
            ..debug
        },
    }
}

fn trust_bonus(trust: TrustLevel) -> i64 {
    match trust {
        TrustLevel::UserConfirmed => 5,
        TrustLevel::ToolObserved => 4,
        TrustLevel::SystemGenerated => 2,
        TrustLevel::AgentInferred => 1,
        TrustLevel::ExternalUntrusted => -3,
    }
}

fn status_bonus(status: MemoryStatus) -> i64 {
    match status {
        MemoryStatus::Active | MemoryStatus::Verified => 5,
        MemoryStatus::Temporary => 0,
        MemoryStatus::Unverified => -1,
        MemoryStatus::Deprecated | MemoryStatus::Superseded => -10,
        MemoryStatus::Rejected => -100,
    }
}

fn sensitivity_penalty(sensitivity: SensitivityLevel) -> i64 {
    match sensitivity {
        SensitivityLevel::Public => 0,
        SensitivityLevel::Private => 1,
        SensitivityLevel::Confidential => 2,
        SensitivityLevel::SecretReference => 100,
    }
}

fn exact_canonical_match(left: &str, right: &str) -> bool {
    let left = canonicalize_marker_value(left);
    let right = canonicalize_marker_value(right);
    !left.is_empty() && left == right
}
