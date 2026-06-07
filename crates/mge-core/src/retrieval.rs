use std::collections::HashSet;

use crate::errors::Result;
use crate::markers::{canonicalize_marker_value, tokenize_keywords, MarkerDictionary};
use crate::models::{MemoryCell, MemoryKind, MemoryStatus, SensitivityLevel, TrustLevel};
use crate::packet::{ContextDebugInfo, ContextMemoryItem, ContextPacket};

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
        }
    }
}

#[derive(Clone, Debug)]
pub struct RankedCell {
    pub cell: MemoryCell,
    pub score: i64,
}

pub fn score_cell(
    cell: &MemoryCell,
    request: &RecallRequest,
    query_marker_ids: &[u32],
    query_tokens: &[String],
) -> Option<i64> {
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

    if !request.include_deprecated
        && matches!(
            cell.status,
            MemoryStatus::Deprecated | MemoryStatus::Rejected
        )
    {
        return None;
    }

    if !request.include_secret_references && cell.sensitivity == SensitivityLevel::SecretReference {
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

    let exact_subject_match = cell.subject.as_ref().is_some_and(|subject| {
        let subject_tokens = tokenize_keywords(subject);
        !subject_tokens.is_empty()
            && subject_tokens
                .iter()
                .all(|token| query_token_set.contains(token.as_str()))
    });

    let mut relevance = marker_overlap * 10;
    if exact_subject_match {
        relevance += 5;
    }
    if value_overlap > 0 {
        relevance += value_overlap.min(3) * 3;
    }

    if relevance <= 0 {
        return None;
    }

    let score = relevance + trust_bonus(cell.trust) + status_bonus(cell.status)
        - sensitivity_penalty(cell.sensitivity);

    Some(score)
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

    let mut warnings = Vec::new();
    if relevant_memory.is_empty() {
        warnings.push("No relevant memory matched the query.".to_string());
    }

    ContextPacket {
        query,
        relevant_memory,
        constraints: vec![
            "Do not use deprecated or rejected memories.".to_string(),
            "Do not expose secret_reference cells.".to_string(),
        ],
        warnings,
        debug,
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
