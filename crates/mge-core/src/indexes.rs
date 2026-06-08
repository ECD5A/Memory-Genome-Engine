use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use xorf::{BinaryFuse16, Filter};

use crate::binary::{self, FileKind};
use crate::errors::{MgeError, Result};
use crate::models::{MarkerId, PageId};
use crate::pages::MemoryPage;

pub trait CandidatePageIndex {
    fn build(pages: &[MemoryPage]) -> Result<Self>
    where
        Self: Sized;

    fn query(&self, markers: &[MarkerId]) -> Result<Vec<PageId>>;

    fn query_with_stats(&self, markers: &[MarkerId]) -> Result<CandidatePageQueryResult> {
        let page_ids = self.query(markers)?;
        Ok(CandidatePageQueryResult {
            page_filters_scanned: 0,
            candidate_pages_returned: page_ids.len(),
            page_ids,
        })
    }

    fn kind(&self) -> IndexKind;
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CandidatePageQueryResult {
    pub page_ids: Vec<PageId>,
    pub page_filters_scanned: usize,
    pub candidate_pages_returned: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexKind {
    ExactMarkerPage,
    BinaryFusePage,
}

impl IndexKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExactMarkerPage => "exact_marker_page",
            Self::BinaryFusePage => "binary_fuse_page",
        }
    }
}

impl Default for IndexKind {
    fn default() -> Self {
        Self::ExactMarkerPage
    }
}

impl fmt::Display for IndexKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for IndexKind {
    type Err = MgeError;

    fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "exact_marker_page" | "exact" | "exact_marker_page_index" => Ok(Self::ExactMarkerPage),
            "binary_fuse_page" | "binary_fuse" | "binary_fuse_page_index" => {
                Ok(Self::BinaryFusePage)
            }
            other => Err(MgeError::InvalidInput(format!(
                "unknown index kind: {other}; supported: exact_marker_page, binary_fuse_page"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "index_kind", content = "data", rename_all = "snake_case")]
pub enum CandidateIndexData {
    ExactMarkerPage(ExactMarkerPageIndex),
    BinaryFusePage(BinaryFusePageIndex),
}

impl CandidatePageIndex for CandidateIndexData {
    fn build(pages: &[MemoryPage]) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(Self::ExactMarkerPage(ExactMarkerPageIndex::build(pages)?))
    }

    fn query(&self, markers: &[MarkerId]) -> Result<Vec<PageId>> {
        match self {
            Self::ExactMarkerPage(index) => index.query(markers),
            Self::BinaryFusePage(index) => index.query(markers),
        }
    }

    fn query_with_stats(&self, markers: &[MarkerId]) -> Result<CandidatePageQueryResult> {
        match self {
            Self::ExactMarkerPage(index) => index.query_with_stats(markers),
            Self::BinaryFusePage(index) => index.query_with_stats(markers),
        }
    }

    fn kind(&self) -> IndexKind {
        match self {
            Self::ExactMarkerPage(index) => index.kind(),
            Self::BinaryFusePage(index) => index.kind(),
        }
    }
}

impl CandidateIndexData {
    pub fn query_with_mode_stats(
        &self,
        markers: &[MarkerId],
        mode: QueryMode,
    ) -> Result<CandidatePageQueryResult> {
        match self {
            Self::ExactMarkerPage(index) => {
                let page_ids = index.query_with_mode(markers, mode);
                Ok(CandidatePageQueryResult {
                    page_filters_scanned: 0,
                    candidate_pages_returned: page_ids.len(),
                    page_ids,
                })
            }
            Self::BinaryFusePage(index) => {
                let page_filters_scanned = if markers.is_empty() {
                    0
                } else {
                    index.page_filters.len()
                };
                let page_ids = index.query_with_mode(markers, mode);
                Ok(CandidatePageQueryResult {
                    page_filters_scanned,
                    candidate_pages_returned: page_ids.len(),
                    page_ids,
                })
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryMode {
    Union,
    Intersection,
    PreferIntersection,
}

/// Per-page static candidate filter backed by the real `xorf::BinaryFuse16`
/// implementation. It is probabilistic and may return extra candidate pages,
/// so the default query path uses union semantics to avoid false negatives.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BinaryFusePageIndex {
    #[serde(default)]
    pub index_kind: IndexKind,
    pub page_filters: Vec<BinaryFusePageFilter>,
    pub all_pages: Vec<PageId>,
}

impl Default for BinaryFusePageIndex {
    fn default() -> Self {
        Self {
            index_kind: IndexKind::BinaryFusePage,
            page_filters: Vec::new(),
            all_pages: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BinaryFusePageFilter {
    pub page_id: PageId,
    pub marker_count: usize,
    pub filter: Option<BinaryFuse16>,
}

impl CandidatePageIndex for BinaryFusePageIndex {
    fn build(pages: &[MemoryPage]) -> Result<Self> {
        let mut all_pages = Vec::with_capacity(pages.len());
        let mut page_filters = Vec::with_capacity(pages.len());

        for page in pages {
            all_pages.push(page.page_id);
            let keys = marker_keys(&page.marker_summary);
            let filter = if keys.is_empty() {
                None
            } else {
                Some(BinaryFuse16::try_from(&keys).map_err(|err| {
                    MgeError::InvalidInput(format!(
                        "binary fuse index build failed for page {}: {err}",
                        page.page_id
                    ))
                })?)
            };

            page_filters.push(BinaryFusePageFilter {
                page_id: page.page_id,
                marker_count: keys.len(),
                filter,
            });
        }

        Ok(Self {
            index_kind: IndexKind::BinaryFusePage,
            page_filters,
            all_pages,
        })
    }

    fn query(&self, markers: &[MarkerId]) -> Result<Vec<PageId>> {
        Ok(self.query_with_mode(markers, QueryMode::Union))
    }

    fn query_with_stats(&self, markers: &[MarkerId]) -> Result<CandidatePageQueryResult> {
        let page_filters_scanned = if markers.is_empty() {
            0
        } else {
            self.page_filters.len()
        };
        let page_ids = self.query(markers)?;
        Ok(CandidatePageQueryResult {
            page_filters_scanned,
            candidate_pages_returned: page_ids.len(),
            page_ids,
        })
    }

    fn kind(&self) -> IndexKind {
        self.index_kind
    }
}

impl BinaryFusePageIndex {
    pub fn query_with_mode(&self, markers: &[MarkerId], mode: QueryMode) -> Vec<PageId> {
        if markers.is_empty() {
            return self.all_pages.clone();
        }

        match mode {
            QueryMode::Union => self.union(markers),
            QueryMode::Intersection => self.intersection(markers),
            QueryMode::PreferIntersection => {
                let intersection = self.intersection(markers);
                if intersection.is_empty() {
                    self.union(markers)
                } else {
                    intersection
                }
            }
        }
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<()> {
        binary::write_messagepack_file(path, FileKind::FuseIndex, self)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }
        binary::read_messagepack_file(path, FileKind::FuseIndex)
    }

    fn union(&self, markers: &[MarkerId]) -> Vec<PageId> {
        let keys = marker_keys(markers);
        self.page_filters
            .iter()
            .filter(|page| page.probably_contains_any(&keys))
            .map(|page| page.page_id)
            .collect()
    }

    fn intersection(&self, markers: &[MarkerId]) -> Vec<PageId> {
        let keys = marker_keys(markers);
        self.page_filters
            .iter()
            .filter(|page| page.probably_contains_all(&keys))
            .map(|page| page.page_id)
            .collect()
    }
}

impl BinaryFusePageFilter {
    fn probably_contains_all(&self, keys: &[u64]) -> bool {
        let Some(filter) = &self.filter else {
            return false;
        };
        keys.iter().all(|key| filter.contains(key))
    }

    fn probably_contains_any(&self, keys: &[u64]) -> bool {
        let Some(filter) = &self.filter else {
            return false;
        };
        keys.iter().any(|key| filter.contains(key))
    }
}

fn marker_keys(markers: &[MarkerId]) -> Vec<u64> {
    let mut keys = markers
        .iter()
        .map(|marker| u64::from(*marker))
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ExactMarkerPageIndex {
    #[serde(default)]
    pub index_kind: IndexKind,
    pub marker_to_pages: BTreeMap<MarkerId, Vec<PageId>>,
    pub all_pages: Vec<PageId>,
}

impl CandidatePageIndex for ExactMarkerPageIndex {
    fn build(pages: &[MemoryPage]) -> Result<Self> {
        let mut marker_to_pages: BTreeMap<MarkerId, BTreeSet<PageId>> = BTreeMap::new();
        let mut all_pages = BTreeSet::new();

        for page in pages {
            all_pages.insert(page.page_id);
            for marker in &page.marker_summary {
                marker_to_pages
                    .entry(*marker)
                    .or_default()
                    .insert(page.page_id);
            }
        }

        Ok(Self {
            index_kind: IndexKind::ExactMarkerPage,
            marker_to_pages: marker_to_pages
                .into_iter()
                .map(|(marker, pages)| (marker, pages.into_iter().collect()))
                .collect(),
            all_pages: all_pages.into_iter().collect(),
        })
    }

    fn query(&self, markers: &[MarkerId]) -> Result<Vec<PageId>> {
        Ok(self.query_with_mode(markers, QueryMode::PreferIntersection))
    }

    fn query_with_stats(&self, markers: &[MarkerId]) -> Result<CandidatePageQueryResult> {
        let page_ids = self.query(markers)?;
        Ok(CandidatePageQueryResult {
            page_filters_scanned: 0,
            candidate_pages_returned: page_ids.len(),
            page_ids,
        })
    }

    fn kind(&self) -> IndexKind {
        self.index_kind
    }
}

impl ExactMarkerPageIndex {
    pub fn query_with_mode(&self, markers: &[MarkerId], mode: QueryMode) -> Vec<PageId> {
        if markers.is_empty() {
            return self.all_pages.clone();
        }

        match mode {
            QueryMode::Union => self.union(markers),
            QueryMode::Intersection => self.intersection(markers),
            QueryMode::PreferIntersection => {
                let intersection = self.intersection(markers);
                if intersection.is_empty() {
                    self.union(markers)
                } else {
                    intersection
                }
            }
        }
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<()> {
        binary::write_messagepack_file(path, FileKind::MarkerIndex, self)
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }
        binary::read_messagepack_file(path, FileKind::MarkerIndex)
    }

    fn union(&self, markers: &[MarkerId]) -> Vec<PageId> {
        let mut pages = BTreeSet::new();
        for marker in markers {
            if let Some(page_ids) = self.marker_to_pages.get(marker) {
                pages.extend(page_ids.iter().copied());
            }
        }
        pages.into_iter().collect()
    }

    fn intersection(&self, markers: &[MarkerId]) -> Vec<PageId> {
        let mut iter = markers.iter();
        let Some(first) = iter.next() else {
            return self.all_pages.clone();
        };

        let Some(first_pages) = self.marker_to_pages.get(first) else {
            return Vec::new();
        };
        let mut intersection: BTreeSet<PageId> = first_pages.iter().copied().collect();

        for marker in iter {
            let Some(page_ids) = self.marker_to_pages.get(marker) else {
                return Vec::new();
            };
            let page_set: BTreeSet<PageId> = page_ids.iter().copied().collect();
            intersection = intersection
                .intersection(&page_set)
                .copied()
                .collect::<BTreeSet<_>>();
            if intersection.is_empty() {
                return Vec::new();
            }
        }

        intersection.into_iter().collect()
    }
}
