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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::compression::CompressionKind;
use crate::errors::{MgeError, Result};
use crate::indexes::IndexKind;
use crate::models::{
    current_timestamp, MarkerId, MemoryCell, MemoryStatus, PageId, SensitivityLevel, TrustLevel,
};

pub const DEFAULT_TARGET_PAGE_BYTES: usize = 64 * 1024;
pub const DEFAULT_MAX_CELLS_PER_PAGE: usize = 512;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MemoryPage {
    pub page_id: PageId,
    pub version: u32,
    pub created_at: i64,
    pub marker_summary: Vec<MarkerId>,
    pub cell_count: usize,
    pub cells: Vec<MemoryCell>,
    pub checksum: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PageCatalog {
    #[serde(default)]
    pub index_kind: IndexKind,
    pub pages: Vec<PageCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PageCatalogEntry {
    pub page_id: PageId,
    pub file: String,
    #[serde(default)]
    pub page_codec: PageCodecKind,
    #[serde(default)]
    pub compression: CompressionKind,
    #[serde(default)]
    pub page_clusterer: PageClustererKind,
    pub created_at: i64,
    pub cell_count: usize,
    pub marker_summary: Vec<MarkerId>,
    #[serde(default)]
    pub scope_marker_summary: Vec<MarkerId>,
    #[serde(default)]
    pub kind_marker_summary: Vec<MarkerId>,
    #[serde(default)]
    pub status_summary: Vec<MemoryStatus>,
    #[serde(default)]
    pub sensitivity_summary: Vec<SensitivityLevel>,
    #[serde(default)]
    pub trust_summary: Vec<TrustLevel>,
    #[serde(default)]
    pub encoded_size_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageCodecKind {
    Json,
    #[default]
    MessagePack,
}

impl PageCodecKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::MessagePack => "messagepack",
        }
    }
}

impl fmt::Display for PageCodecKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PageCodecKind {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "messagepack" | "msgpack" | "mpack" => Ok(Self::MessagePack),
            _ => Err(MgeError::InvalidInput(format!(
                "unknown page codec kind: {input}"
            ))),
        }
    }
}

pub trait PageCodec {
    fn encode(&self, page: &MemoryPage) -> Result<Vec<u8>>;
    fn decode(&self, bytes: &[u8]) -> Result<MemoryPage>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct JsonPageCodec;

impl PageCodec for JsonPageCodec {
    fn encode(&self, page: &MemoryPage) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(page)?)
    }

    fn decode(&self, bytes: &[u8]) -> Result<MemoryPage> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MessagePackPageCodec;

impl PageCodec for MessagePackPageCodec {
    fn encode(&self, page: &MemoryPage) -> Result<Vec<u8>> {
        Ok(rmp_serde::to_vec_named(page)?)
    }

    fn decode(&self, bytes: &[u8]) -> Result<MemoryPage> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}

pub fn encode_page_with(kind: PageCodecKind, page: &MemoryPage) -> Result<Vec<u8>> {
    match kind {
        PageCodecKind::Json => JsonPageCodec.encode(page),
        PageCodecKind::MessagePack => MessagePackPageCodec.encode(page),
    }
}

pub fn decode_page_with(kind: PageCodecKind, bytes: &[u8]) -> Result<MemoryPage> {
    match kind {
        PageCodecKind::Json => JsonPageCodec.decode(bytes),
        PageCodecKind::MessagePack => MessagePackPageCodec.decode(bytes),
    }
}

pub fn attach_page_checksum(page: &mut MemoryPage) -> Result<()> {
    page.checksum = Some(page_content_checksum(page)?);
    Ok(())
}

pub fn page_content_checksum(page: &MemoryPage) -> Result<String> {
    let mut canonical = page.clone();
    canonical.checksum = None;
    let bytes = rmp_serde::to_vec_named(&canonical)?;
    Ok(sha256_hex(&bytes))
}

pub fn page_checksum_matches(page: &MemoryPage) -> Result<bool> {
    let Some(expected) = &page.checksum else {
        return Ok(true);
    };
    Ok(expected == &page_content_checksum(page)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageBuildOptions {
    pub target_page_bytes: usize,
    pub max_cells_per_page: usize,
}

impl Default for PageBuildOptions {
    fn default() -> Self {
        Self {
            target_page_bytes: DEFAULT_TARGET_PAGE_BYTES,
            max_cells_per_page: DEFAULT_MAX_CELLS_PER_PAGE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageClustererKind {
    #[default]
    ScopeKind,
    MarkerOverlap,
}

impl PageClustererKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ScopeKind => "scope_kind",
            Self::MarkerOverlap => "marker_overlap",
        }
    }
}

impl fmt::Display for PageClustererKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PageClustererKind {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "scope_kind" | "scope-kind" | "scope+kind" | "scope_kind_clusterer" => {
                Ok(Self::ScopeKind)
            }
            "marker_overlap" | "marker-overlap" | "marker_overlap_clusterer" => {
                Ok(Self::MarkerOverlap)
            }
            other => Err(MgeError::InvalidInput(format!(
                "unknown page clusterer kind: {other}"
            ))),
        }
    }
}

pub trait PageClusterer {
    fn cluster_cells(&self, cells: &[MemoryCell]) -> Vec<Vec<MemoryCell>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ScopeKindClusterer;

impl PageClusterer for ScopeKindClusterer {
    fn cluster_cells(&self, cells: &[MemoryCell]) -> Vec<Vec<MemoryCell>> {
        let mut groups: BTreeMap<(String, String), Vec<MemoryCell>> = BTreeMap::new();
        for cell in cells {
            groups
                .entry((cell.scope.clone(), cell.kind.as_str().to_string()))
                .or_default()
                .push(cell.clone());
        }

        groups.into_values().collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MarkerOverlapClusterer {
    pub min_overlap: usize,
}

impl Default for MarkerOverlapClusterer {
    fn default() -> Self {
        Self { min_overlap: 4 }
    }
}

impl MarkerOverlapClusterer {
    pub fn new(min_overlap: usize) -> Self {
        Self { min_overlap }
    }
}

impl PageClusterer for MarkerOverlapClusterer {
    fn cluster_cells(&self, cells: &[MemoryCell]) -> Vec<Vec<MemoryCell>> {
        let base_groups = ScopeKindClusterer.cluster_cells(cells);
        let mut output = Vec::new();

        for mut group in base_groups {
            group.sort_by_key(|cell| cell.id);
            let mut clusters: Vec<(BTreeSet<MarkerId>, Vec<MemoryCell>)> = Vec::new();

            for cell in group {
                let mut marker_set = BTreeSet::new();
                cell.for_each_marker_id_for_indexing(|marker_id| {
                    marker_set.insert(marker_id);
                });
                let best_cluster = clusters
                    .iter()
                    .enumerate()
                    .map(|(index, (cluster_markers, _))| {
                        let overlap = marker_set.intersection(cluster_markers).count();
                        (index, overlap)
                    })
                    .max_by_key(|(_, overlap)| *overlap);

                if let Some((index, overlap)) = best_cluster {
                    if overlap >= self.min_overlap {
                        clusters[index].0.extend(marker_set);
                        clusters[index].1.push(cell);
                        continue;
                    }
                }

                clusters.push((marker_set, vec![cell]));
            }

            output.extend(clusters.into_iter().map(|(_, cells)| cells));
        }

        output
    }
}

pub fn build_pages_from_cells(cells: &[MemoryCell], start_page_id: PageId) -> Vec<MemoryPage> {
    build_pages_with_kind(
        cells,
        start_page_id,
        PageClustererKind::ScopeKind,
        PageBuildOptions::default(),
    )
}

pub fn build_pages_with_kind(
    cells: &[MemoryCell],
    start_page_id: PageId,
    kind: PageClustererKind,
    options: PageBuildOptions,
) -> Vec<MemoryPage> {
    match kind {
        PageClustererKind::ScopeKind => {
            build_pages_with_clusterer(cells, start_page_id, &ScopeKindClusterer, options)
        }
        PageClustererKind::MarkerOverlap => build_pages_with_clusterer(
            cells,
            start_page_id,
            &MarkerOverlapClusterer::default(),
            options,
        ),
    }
}

pub fn build_pages_with_default_clusterer(
    cells: &[MemoryCell],
    start_page_id: PageId,
) -> Vec<MemoryPage> {
    build_pages_with_clusterer(
        cells,
        start_page_id,
        &ScopeKindClusterer,
        PageBuildOptions::default(),
    )
}

pub fn build_pages_with_clusterer<C: PageClusterer>(
    cells: &[MemoryCell],
    start_page_id: PageId,
    clusterer: &C,
    options: PageBuildOptions,
) -> Vec<MemoryPage> {
    let mut next_page_id = start_page_id;
    let mut pages = Vec::new();

    for group in clusterer.cluster_cells(cells) {
        for page_cells in split_group_by_limits(group, options) {
            let marker_summary = marker_summary_for_cells(&page_cells);
            pages.push(MemoryPage {
                page_id: next_page_id,
                version: 1,
                created_at: current_timestamp(),
                marker_summary,
                cell_count: page_cells.len(),
                cells: page_cells,
                checksum: None,
            });
            next_page_id += 1;
        }
    }

    pages
}

pub fn page_file_name(page_id: PageId) -> String {
    format!("{page_id:06}.mgp")
}

fn marker_summary_for_cells(cells: &[MemoryCell]) -> Vec<MarkerId> {
    crate::models::MarkerGenome::marker_summary(cells)
}

fn split_group_by_limits(
    group: Vec<MemoryCell>,
    options: PageBuildOptions,
) -> Vec<Vec<MemoryCell>> {
    let max_cells = options.max_cells_per_page.max(1);
    let target_bytes = options.target_page_bytes.max(1);
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0usize;

    for cell in group {
        let cell_bytes = estimate_cell_bytes(&cell);
        let would_exceed_cells = current.len() >= max_cells;
        let would_exceed_bytes = !current.is_empty() && current_bytes + cell_bytes > target_bytes;

        if would_exceed_cells || would_exceed_bytes {
            chunks.push(current);
            current = Vec::new();
            current_bytes = 0;
        }

        current_bytes += cell_bytes;
        current.push(cell);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn estimate_cell_bytes(cell: &MemoryCell) -> usize {
    rmp_serde::to_vec_named(cell)
        .map(|bytes| bytes.len())
        .unwrap_or(DEFAULT_TARGET_PAGE_BYTES)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}
