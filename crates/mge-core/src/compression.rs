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

use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Cursor;
use std::str::FromStr;

use crate::errors::MgeError;

const DEFAULT_ZSTD_LEVEL: i32 = 3;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionKind {
    #[default]
    None,
    Zstd,
}

impl CompressionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zstd => "zstd",
        }
    }
}

impl fmt::Display for CompressionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for CompressionKind {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "none" | "no_compression" | "nocompression" => Ok(Self::None),
            "zstd" => Ok(Self::Zstd),
            _ => Err(MgeError::InvalidInput(format!(
                "unknown compression kind: {input}"
            ))),
        }
    }
}

pub trait Compressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoCompression;

impl Compressor for NoCompression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(data.to_vec())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ZstdCompression {
    level: i32,
}

impl Default for ZstdCompression {
    fn default() -> Self {
        Self {
            level: DEFAULT_ZSTD_LEVEL,
        }
    }
}

impl ZstdCompression {
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl Compressor for ZstdCompression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(zstd::stream::encode_all(Cursor::new(data), self.level)?)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        Ok(zstd::stream::decode_all(Cursor::new(data))?)
    }
}

pub fn compress_with(kind: CompressionKind, data: &[u8]) -> Result<Vec<u8>> {
    match kind {
        CompressionKind::None => NoCompression.compress(data),
        CompressionKind::Zstd => ZstdCompression::default().compress(data),
    }
}

pub fn decompress_with(kind: CompressionKind, data: &[u8]) -> Result<Vec<u8>> {
    match kind {
        CompressionKind::None => NoCompression.decompress(data),
        CompressionKind::Zstd => ZstdCompression::default().decompress(data),
    }
}
