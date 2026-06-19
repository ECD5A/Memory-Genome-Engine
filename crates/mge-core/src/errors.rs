// Memory Genome Engine
// Copyright (c) 2026 ECD5A
// Project: https://github.com/ECD5A/Memory-Genome-Engine
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MgeError>;

#[derive(Debug, Error)]
pub enum MgeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("messagepack encode error: {0}")]
    MessagePackEncode(#[from] rmp_serde::encode::Error),

    #[error("messagepack decode error: {0}")]
    MessagePackDecode(#[from] rmp_serde::decode::Error),

    #[error("storage format error: {0}")]
    StorageFormat(String),

    #[error("invalid marker: {0}")]
    InvalidMarker(String),

    #[error("store is not initialized: {0}")]
    NotInitialized(String),

    #[error("store is locked: {0}")]
    StoreLocked(String),

    #[error("store is busy: {0}")]
    StoreBusy(String),

    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),
}
