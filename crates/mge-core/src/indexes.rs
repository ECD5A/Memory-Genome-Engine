use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::errors::{MgeError, Result};
use crate::models::{MarkerId, PageId};
use crate::pages::MemoryPage;

pub trait CandidatePageIndex {
    fn build(pages: &[MemoryPage]) -> Result<Self>
    where
        Self: Sized;

    fn query(&self, markers: &[MarkerId]) -> Result<Vec<PageId>>;

    fn kind(&self) -> IndexKind;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexKind {
    ExactMarkerPage,
}

impl IndexKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExactMarkerPage => "exact_marker_page",
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
            other => Err(MgeError::InvalidInput(format!(
                "unknown index kind: {other}; v0.1 supports exact_marker_page only"
            ))),
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
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_slice(&fs::read(path)?)?)
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
