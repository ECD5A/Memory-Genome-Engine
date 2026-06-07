use serde::{Deserialize, Serialize};

use crate::models::{MemoryKind, MemoryStatus, PageId, SensitivityLevel, TrustLevel};

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
    pub hot_cells_scanned: usize,
    pub candidate_pages: Vec<PageId>,
    pub sealed_cells_scanned: usize,
    pub total_candidates: usize,
}
