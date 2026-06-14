use std::collections::HashSet;

use crate::errors::Result;
use crate::markers::{canonicalize_marker_value, tokenize_keywords, MarkerDictionary};
use crate::models::{
    MemoryCell, MemoryKind, MemoryStatus, RecallMode, SensitivityLevel, TrustLevel,
};
use crate::packet::{ContextDebugInfo, ContextMemoryItem, ContextPacket, ContextScoreDebugItem};
use crate::security::{AgentCapabilities, RecallPolicy};

pub trait Retriever {
    fn recall(&self, request: RecallRequest) -> Result<ContextPacket>;
}

#[derive(Clone, Debug)]
pub struct RecallRequest {
    pub query: String,
    pub mode: RecallMode,
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
            mode: RecallMode::Focused,
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

    pub fn effective_max_items(&self, total_candidates: usize) -> usize {
        match self.mode {
            RecallMode::Focused => self.max_items,
            RecallMode::Broad => self.max_items.max(20),
            RecallMode::FullScope => total_candidates,
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

#[derive(Clone, Debug)]
pub struct RecallFilterContext {
    kind: Option<MemoryKind>,
    scope_raw: Option<String>,
    scope_canonical: Option<String>,
    scope_marker_id: Option<u32>,
    required_cell_marker_ids: Vec<u32>,
    policy: RecallPolicy,
}

impl RecallFilterContext {
    pub fn new(request: &RecallRequest) -> Self {
        Self::new_with_marker_filters(request, None, Vec::new())
    }

    pub fn new_with_marker_filters(
        request: &RecallRequest,
        scope_marker_id: Option<u32>,
        required_cell_marker_ids: Vec<u32>,
    ) -> Self {
        Self {
            kind: request.kind,
            scope_raw: request.scope.clone(),
            scope_canonical: request.scope.as_deref().map(canonicalize_marker_value),
            scope_marker_id,
            required_cell_marker_ids,
            policy: request.effective_policy(),
        }
    }

    pub fn permits_cell(&self, cell: &MemoryCell) -> bool {
        if let Some(kind) = self.kind {
            if cell.kind != kind {
                return false;
            }
        }

        if let Some(scope_marker_id) = self.scope_marker_id {
            if !cell.contains_marker(scope_marker_id) {
                return false;
            }
        }

        if !self.required_cell_marker_ids.is_empty()
            && !self
                .required_cell_marker_ids
                .iter()
                .any(|marker| cell.contains_marker(*marker))
        {
            return false;
        }

        if let Some(scope) = &self.scope_canonical {
            if self
                .scope_raw
                .as_ref()
                .is_none_or(|raw_scope| cell.scope != *raw_scope)
                && canonicalize_marker_value(&cell.scope) != *scope
            {
                return false;
            }
        }

        self.policy.permits_cell(cell)
    }
}

#[derive(Clone, Debug)]
pub struct ScoringContext {
    filter: RecallFilterContext,
    query_marker_set: HashSet<u32>,
    query_token_set: HashSet<String>,
    query_canonical: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CachedCellScoringData {
    pub value_tokens: Vec<String>,
    pub value_canonical: String,
    pub subject_tokens: Vec<String>,
}

impl CachedCellScoringData {
    pub(crate) fn from_cell(cell: &MemoryCell) -> Self {
        let value_text = cell.value.to_plain_text_cow();
        Self {
            value_tokens: tokenize_keywords(value_text.as_ref()),
            value_canonical: canonicalize_marker_value(value_text.as_ref()),
            subject_tokens: cell
                .subject
                .as_deref()
                .map(tokenize_keywords)
                .unwrap_or_default(),
        }
    }
}

impl ScoringContext {
    pub fn new(request: &RecallRequest, query_marker_ids: &[u32], query_tokens: &[String]) -> Self {
        Self::new_with_filter(
            request,
            RecallFilterContext::new(request),
            query_marker_ids,
            query_tokens,
        )
    }

    pub fn new_with_filter(
        request: &RecallRequest,
        filter: RecallFilterContext,
        query_marker_ids: &[u32],
        query_tokens: &[String],
    ) -> Self {
        Self {
            filter,
            query_marker_set: query_marker_ids.iter().copied().collect(),
            query_token_set: query_tokens.iter().cloned().collect(),
            query_canonical: canonicalize_marker_value(&request.query),
        }
    }
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
    let context = ScoringContext::new(request, query_marker_ids, query_tokens);
    score_cell_debug_with_context(cell, &context)
}

pub fn score_cell_debug_with_context(
    cell: &MemoryCell,
    context: &ScoringContext,
) -> Option<ContextScoreDebugItem> {
    if !context.filter.permits_cell(cell) {
        return None;
    }

    let cached = CachedCellScoringData::from_cell(cell);
    score_permitted_cell_debug(cell, context, &cached)
}

pub(crate) fn score_cell_debug_with_cached_context(
    cell: &MemoryCell,
    context: &ScoringContext,
    cached: &CachedCellScoringData,
) -> Option<ContextScoreDebugItem> {
    if !context.filter.permits_cell(cell) {
        return None;
    }

    score_permitted_cell_debug(cell, context, cached)
}

fn score_permitted_cell_debug(
    cell: &MemoryCell,
    context: &ScoringContext,
    cached: &CachedCellScoringData,
) -> Option<ContextScoreDebugItem> {
    let marker_overlap = cell.marker_overlap_count(&context.query_marker_set) as i64;

    let value_overlap = if context.query_token_set.is_empty() {
        0
    } else {
        cached
            .value_tokens
            .iter()
            .filter(|token| context.query_token_set.contains(token.as_str()))
            .count() as i64
    };
    let exact_value_match =
        !cached.value_canonical.is_empty() && cached.value_canonical == context.query_canonical;

    let exact_subject_match = !context.query_token_set.is_empty()
        && !cached.subject_tokens.is_empty()
        && cached
            .subject_tokens
            .iter()
            .all(|token| context.query_token_set.contains(token.as_str()));

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

pub fn full_scope_cell_debug(
    cell: &MemoryCell,
    request: &RecallRequest,
) -> Option<ContextScoreDebugItem> {
    let filter = RecallFilterContext::new(request);
    full_scope_cell_debug_with_filter(cell, &filter)
}

pub fn full_scope_cell_debug_with_filter(
    cell: &MemoryCell,
    filter: &RecallFilterContext,
) -> Option<ContextScoreDebugItem> {
    if !filter.permits_cell(cell) {
        return None;
    }

    let trust_bonus = trust_bonus(cell.trust);
    let status_bonus = status_bonus(cell.status);
    let sensitivity_penalty = sensitivity_penalty(cell.sensitivity);
    let score = trust_bonus + status_bonus - sensitivity_penalty;

    Some(ContextScoreDebugItem {
        cell_id: cell.id,
        score,
        trust_bonus,
        status_bonus,
        sensitivity_penalty,
        ..Default::default()
    })
}

pub fn build_context_packet(
    query: String,
    ranked: &[RankedCell],
    dictionary: &MarkerDictionary,
    debug: ContextDebugInfo,
    max_items: usize,
) -> ContextPacket {
    let mut seen_cell_ids = HashSet::new();
    let mut total_candidates = 0;
    let mut relevant_memory = Vec::with_capacity(max_items.min(ranked.len()));
    let mut score_details = Vec::with_capacity(max_items.min(ranked.len()));

    for ranked in ranked {
        if !seen_cell_ids.insert(ranked.cell.id) {
            continue;
        }
        total_candidates += 1;
        if relevant_memory.len() < max_items {
            let mut seen_marker_ids = std::collections::BTreeSet::new();
            let mut markers = Vec::new();
            ranked.cell.for_each_marker_id_for_indexing(|marker_id| {
                if seen_marker_ids.insert(marker_id) {
                    if let Some(marker) = dictionary.marker(marker_id) {
                        markers.push(marker.to_string());
                    }
                }
            });

            relevant_memory.push(ContextMemoryItem {
                kind: ranked.cell.kind,
                content: ranked.cell.value.to_plain_text(),
                trust: ranked.cell.trust,
                status: ranked.cell.status,
                scope: ranked.cell.scope.clone(),
                sensitivity: ranked.cell.sensitivity,
                markers,
            });
            score_details.push(ranked.score_detail.clone());
        }
    }

    let includes_deprecated_or_rejected = relevant_memory.iter().any(|item| {
        matches!(
            item.status,
            MemoryStatus::Deprecated | MemoryStatus::Rejected | MemoryStatus::Superseded
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
        warnings.push(
            "Deprecated, rejected, or superseded memories were included by explicit policy."
                .to_string(),
        );
    } else {
        constraints.push("Do not use deprecated, rejected, or superseded memories.".to_string());
    }
    if includes_secret_references {
        warnings.push("SecretReference cells were included by explicit policy.".to_string());
    } else {
        constraints.push("Do not expose secret_reference cells.".to_string());
    }
    let returned_items = relevant_memory.len();

    ContextPacket {
        query,
        relevant_memory,
        constraints,
        warnings,
        debug: ContextDebugInfo {
            total_candidates,
            returned_items,
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
