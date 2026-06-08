use serde::{Deserialize, Serialize};

use crate::indexes::IndexKind;
use crate::models::{
    CellId, MemoryKind, MemoryStatus, PageId, RecallMode, SensitivityLevel, TrustLevel,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContextPacket {
    pub query: String,
    pub relevant_memory: Vec<ContextMemoryItem>,
    pub constraints: Vec<String>,
    pub warnings: Vec<String>,
    pub debug: ContextDebugInfo,
}

impl ContextPacket {
    pub fn to_prompt_text(&self) -> String {
        let mut output = String::new();
        output.push_str("Relevant memory:\n");
        if self.relevant_memory.is_empty() {
            output.push_str("- No relevant memory found.\n");
        } else {
            for item in &self.relevant_memory {
                output.push_str(&format!(
                    "- {} [kind={}, trust={}, status={}, scope={}]\n",
                    item.content, item.kind, item.trust, item.status, item.scope
                ));
            }
        }

        if !self.constraints.is_empty() {
            output.push_str("\nConstraints:\n");
            for constraint in &self.constraints {
                output.push_str(&format!("- {constraint}\n"));
            }
        }

        if !self.warnings.is_empty() {
            output.push_str("\nWarnings:\n");
            for warning in &self.warnings {
                output.push_str(&format!("- {warning}\n"));
            }
        }

        output
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ContextMemoryItem {
    pub kind: MemoryKind,
    pub content: String,
    pub trust: TrustLevel,
    pub status: MemoryStatus,
    pub scope: String,
    pub sensitivity: SensitivityLevel,
    pub markers: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextDebugInfo {
    #[serde(default)]
    pub recall_mode: RecallMode,
    #[serde(default)]
    pub max_items: usize,
    #[serde(default)]
    pub index_kind: IndexKind,
    pub hot_cells_scanned: usize,
    #[serde(default)]
    pub cells_scanned: usize,
    pub candidate_pages: Vec<PageId>,
    pub page_filters_scanned: usize,
    pub candidate_pages_returned: usize,
    pub loaded_pages: usize,
    pub sealed_cells_scanned: usize,
    pub false_positive_candidate_pages: usize,
    pub total_candidates: usize,
    #[serde(default)]
    pub returned_items: usize,
    #[serde(default)]
    pub full_scope_used: bool,
    #[serde(default)]
    pub score_details: Vec<ContextScoreDebugItem>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextScoreDebugItem {
    pub cell_id: CellId,
    pub score: i64,
    pub marker_overlap: i64,
    pub marker_overlap_score: i64,
    pub exact_subject_match: bool,
    pub exact_subject_score: i64,
    pub value_overlap: i64,
    pub value_overlap_score: i64,
    #[serde(default)]
    pub exact_value_match: bool,
    #[serde(default)]
    pub exact_value_score: i64,
    pub trust_bonus: i64,
    pub status_bonus: i64,
    pub sensitivity_penalty: i64,
}
