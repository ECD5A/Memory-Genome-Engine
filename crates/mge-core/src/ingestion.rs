// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0.

use serde::{Deserialize, Serialize};

use crate::errors::{MgeError, Result};
use crate::models::{
    CellId, MemoryCell, MemoryKind, MemorySource, MemoryStatus, SensitivityLevel, TrustLevel,
};

pub const DEFAULT_SESSION_CHUNK_MAX_TURNS: usize = 8;
pub const DEFAULT_SESSION_CHUNK_MAX_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionTurn {
    pub role: String,
    pub content: String,
}

impl SessionTurn {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionChunkOptions {
    pub max_turns: usize,
    pub max_bytes: usize,
}

impl Default for SessionChunkOptions {
    fn default() -> Self {
        Self {
            max_turns: DEFAULT_SESSION_CHUNK_MAX_TURNS,
            max_bytes: DEFAULT_SESSION_CHUNK_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionChunk {
    pub index: usize,
    pub start_turn: usize,
    pub end_turn: usize,
    pub text: String,
}

impl SessionChunk {
    pub fn turn_count(&self) -> usize {
        self.end_turn.saturating_sub(self.start_turn)
    }
}

#[derive(Clone, Debug)]
pub struct SessionRememberRequest {
    pub turns: Vec<SessionTurn>,
    pub chunk_options: SessionChunkOptions,
    pub session_id: Option<String>,
    pub kind: MemoryKind,
    pub subject: Option<String>,
    pub scope: String,
    pub status: MemoryStatus,
    pub trust: TrustLevel,
    pub sensitivity: SensitivityLevel,
    pub markers: Vec<String>,
    pub source: Option<MemorySource>,
    pub links: Vec<CellId>,
}

impl SessionRememberRequest {
    pub fn new(turns: Vec<SessionTurn>) -> Self {
        Self {
            turns,
            chunk_options: SessionChunkOptions::default(),
            session_id: None,
            kind: MemoryKind::ProjectFact,
            subject: None,
            scope: "global".to_string(),
            status: MemoryStatus::Active,
            trust: TrustLevel::ToolObserved,
            sensitivity: SensitivityLevel::Private,
            markers: Vec::new(),
            source: None,
            links: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionRememberReport {
    pub turns: usize,
    pub chunks: usize,
    pub cells: Vec<MemoryCell>,
}

pub fn chunk_session_turns(
    turns: &[SessionTurn],
    options: SessionChunkOptions,
) -> Result<Vec<SessionChunk>> {
    if options.max_turns == 0 {
        return Err(MgeError::InvalidInput(
            "session chunk max_turns must be greater than zero".to_string(),
        ));
    }
    if options.max_bytes == 0 {
        return Err(MgeError::InvalidInput(
            "session chunk max_bytes must be greater than zero".to_string(),
        ));
    }
    if turns.is_empty() {
        return Err(MgeError::InvalidInput(
            "session must contain at least one turn".to_string(),
        ));
    }

    let mut chunks = Vec::new();
    let mut lines = Vec::new();
    let mut chunk_start = 0usize;
    let mut bytes = 0usize;

    for (turn_index, turn) in turns.iter().enumerate() {
        let role = turn.role.trim();
        let content = turn.content.trim();
        if role.is_empty() || content.is_empty() {
            return Err(MgeError::InvalidInput(format!(
                "session turn {turn_index} requires non-empty role and content"
            )));
        }
        let line = format!("{role}: {content}");
        let separator_bytes = usize::from(!lines.is_empty());
        let exceeds_limit = lines.len() >= options.max_turns
            || bytes
                .saturating_add(separator_bytes)
                .saturating_add(line.len())
                > options.max_bytes;
        if !lines.is_empty() && exceeds_limit {
            let index = chunks.len();
            chunks.push(SessionChunk {
                index,
                start_turn: chunk_start,
                end_turn: turn_index,
                text: lines.join("\n"),
            });
            lines.clear();
            bytes = 0;
            chunk_start = turn_index;
        }

        if !lines.is_empty() {
            bytes = bytes.saturating_add(1);
        }
        bytes = bytes.saturating_add(line.len());
        lines.push(line);
    }

    if !lines.is_empty() {
        chunks.push(SessionChunk {
            index: chunks.len(),
            start_turn: chunk_start,
            end_turn: turns.len(),
            text: lines.join("\n"),
        });
    }
    Ok(chunks)
}
