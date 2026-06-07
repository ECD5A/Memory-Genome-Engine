use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::{MgeError, Result};
use crate::models::{
    MarkerId, MemoryKind, MemoryStatus, MemoryValue, SensitivityLevel, TrustLevel,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MarkerDebugEntry {
    pub id: MarkerId,
    pub marker: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MarkerDictionary {
    marker_to_id: BTreeMap<String, MarkerId>,
    id_to_marker: BTreeMap<MarkerId, String>,
    next_id: MarkerId,
}

impl Default for MarkerDictionary {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkerDictionary {
    pub fn new() -> Self {
        Self {
            marker_to_id: BTreeMap::new(),
            id_to_marker: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.marker_to_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.marker_to_id.is_empty()
    }

    pub fn get_or_insert(&mut self, marker: &str) -> Result<MarkerId> {
        let canonical = canonicalize_marker(marker)?;
        if let Some(id) = self.marker_to_id.get(&canonical) {
            return Ok(*id);
        }

        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| MgeError::InvalidInput("marker id overflow".to_string()))?;
        self.marker_to_id.insert(canonical.clone(), id);
        self.id_to_marker.insert(id, canonical);
        Ok(id)
    }

    pub fn lookup(&self, marker: &str) -> Option<MarkerId> {
        canonicalize_marker(marker)
            .ok()
            .and_then(|canonical| self.marker_to_id.get(&canonical).copied())
    }

    pub fn marker(&self, id: MarkerId) -> Option<&str> {
        self.id_to_marker.get(&id).map(String::as_str)
    }

    pub fn debug_view(&self) -> Vec<MarkerDebugEntry> {
        self.id_to_marker
            .iter()
            .map(|(id, marker)| MarkerDebugEntry {
                id: *id,
                marker: marker.clone(),
            })
            .collect()
    }

    pub fn consistency_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for (marker, id) in &self.marker_to_id {
            if *id == 0 {
                errors.push(format!("marker {marker} uses invalid id 0"));
            }
            match canonicalize_marker(marker) {
                Ok(canonical) if canonical == *marker => {}
                Ok(canonical) => errors.push(format!(
                    "marker {marker} is not canonical; expected {canonical}"
                )),
                Err(_) => errors.push(format!("marker {marker} is invalid")),
            }
            match self.id_to_marker.get(id) {
                Some(reverse_marker) if reverse_marker == marker => {}
                Some(reverse_marker) => errors.push(format!(
                    "marker_to_id {marker}->{id} conflicts with id_to_marker {id}->{reverse_marker}"
                )),
                None => errors.push(format!(
                    "marker_to_id {marker}->{id} is missing reverse id_to_marker entry"
                )),
            }
        }

        for (id, marker) in &self.id_to_marker {
            if *id == 0 {
                errors.push(format!(
                    "id_to_marker uses invalid id 0 for marker {marker}"
                ));
            }
            match self.marker_to_id.get(marker) {
                Some(forward_id) if forward_id == id => {}
                Some(forward_id) => errors.push(format!(
                    "id_to_marker {id}->{marker} conflicts with marker_to_id {marker}->{forward_id}"
                )),
                None => errors.push(format!(
                    "id_to_marker {id}->{marker} is missing forward marker_to_id entry"
                )),
            }
        }

        let max_id = self
            .id_to_marker
            .keys()
            .chain(self.marker_to_id.values())
            .copied()
            .max()
            .unwrap_or(0);
        if max_id >= self.next_id {
            errors.push(format!(
                "next_id {} must be greater than max marker id {}",
                self.next_id, max_id
            ));
        }

        errors
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = rmp_serde::to_vec_named(self)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Self::new());
        }
        let bytes = fs::read(path)?;
        let mut dictionary: Self = rmp_serde::from_slice(&bytes)?;
        if dictionary.next_id == 0 {
            dictionary.next_id = dictionary
                .id_to_marker
                .keys()
                .next_back()
                .copied()
                .unwrap_or(0)
                + 1;
        }
        Ok(dictionary)
    }
}

pub fn canonicalize_marker(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(MgeError::InvalidMarker(raw.to_string()));
    }

    let (category, value) = if let Some((category, value)) = trimmed.split_once(':') {
        (
            canonicalize_marker_value(category),
            canonicalize_marker_value(value),
        )
    } else {
        ("tag".to_string(), canonicalize_marker_value(trimmed))
    };

    if category.is_empty() || value.is_empty() {
        return Err(MgeError::InvalidMarker(raw.to_string()));
    }

    Ok(format!("{category}:{value}"))
}

pub fn canonicalize_marker_value(raw: &str) -> String {
    let mut out = String::new();
    let mut previous_was_separator = false;

    for ch in raw.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator {
            out.push('_');
            previous_was_separator = true;
        }
    }

    out.trim_matches('_').to_string()
}

pub fn marker_strings_for_cell_fields(
    kind: &MemoryKind,
    subject: Option<&str>,
    value: &MemoryValue,
    scope: &str,
    status: &MemoryStatus,
    trust: &TrustLevel,
    sensitivity: &SensitivityLevel,
    explicit_markers: &[String],
) -> Result<Vec<String>> {
    let mut markers = Vec::new();

    push_marker(&mut markers, format!("kind:{}", kind.as_str()))?;
    push_marker(
        &mut markers,
        format!("scope:{}", canonicalize_marker_value(scope)),
    )?;
    push_marker(&mut markers, format!("status:{}", status.as_str()))?;
    push_marker(&mut markers, format!("trust:{}", trust.as_str()))?;
    push_marker(
        &mut markers,
        format!("sensitivity:{}", sensitivity.as_str()),
    )?;

    if let Some(subject) = subject {
        push_marker(
            &mut markers,
            format!("subject:{}", canonicalize_marker_value(subject)),
        )?;
        for token in tokenize_keywords(subject) {
            push_marker(&mut markers, format!("tag:{token}"))?;
        }
    }

    match value {
        MemoryValue::Symbol(value) | MemoryValue::Reference(value) => {
            push_marker(
                &mut markers,
                format!("value:{}", canonicalize_marker_value(value)),
            )?;
            for token in tokenize_keywords(value) {
                push_marker(&mut markers, format!("tag:{token}"))?;
            }
        }
        MemoryValue::Text(value) => {
            let tokens = tokenize_keywords(value);
            if value.len() <= 64 && tokens.len() <= 6 {
                push_marker(
                    &mut markers,
                    format!("value:{}", canonicalize_marker_value(value)),
                )?;
            }
            for token in tokens.into_iter().take(16) {
                push_marker(&mut markers, format!("tag:{token}"))?;
            }
        }
        MemoryValue::Number(value) => {
            push_marker(&mut markers, format!("value:{value}"))?;
        }
        MemoryValue::Boolean(value) => {
            push_marker(&mut markers, format!("value:{value}"))?;
        }
        MemoryValue::Timestamp(value) => {
            push_marker(&mut markers, format!("value:{value}"))?;
        }
        MemoryValue::Structured(value) => {
            push_structured_value_markers(&mut markers, value)?;
        }
    }

    for marker in explicit_markers {
        push_marker(&mut markers, marker.clone())?;
    }

    Ok(markers)
}

pub fn extract_query_marker_strings(query: &str) -> Vec<String> {
    tokenize_keywords(query)
        .into_iter()
        .map(|token| format!("tag:{token}"))
        .collect()
}

pub fn tokenize_keywords(text: &str) -> Vec<String> {
    let stopwords = stopwords();
    let mut seen = HashSet::new();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else {
            push_token(&mut tokens, &mut seen, &stopwords, &current);
            current.clear();
        }
    }
    push_token(&mut tokens, &mut seen, &stopwords, &current);

    tokens
}

fn push_token(
    tokens: &mut Vec<String>,
    seen: &mut HashSet<String>,
    stopwords: &HashSet<&'static str>,
    raw: &str,
) {
    if raw.len() < 2 || stopwords.contains(raw) {
        return;
    }

    let token = singularize(raw);
    if token.len() < 2 || stopwords.contains(token.as_str()) {
        return;
    }

    if seen.insert(token.clone()) {
        tokens.push(token);
    }
}

fn singularize(raw: &str) -> String {
    if raw.len() > 4 && raw.ends_with("ies") {
        format!("{}y", &raw[..raw.len() - 3])
    } else if raw.len() > 3 && raw.ends_with('s') && !raw.ends_with("ss") {
        raw[..raw.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}

fn push_marker(markers: &mut Vec<String>, marker: String) -> Result<()> {
    let canonical = canonicalize_marker(&marker)?;
    if !markers.iter().any(|existing| existing == &canonical) {
        markers.push(canonical);
    }
    Ok(())
}

fn push_structured_value_markers(
    markers: &mut Vec<String>,
    value: &serde_json::Value,
) -> Result<()> {
    let mut budget = 16;
    push_structured_value_markers_inner(markers, value, 0, &mut budget)
}

fn push_structured_value_markers_inner(
    markers: &mut Vec<String>,
    value: &serde_json::Value,
    depth: usize,
    budget: &mut usize,
) -> Result<()> {
    if *budget == 0 || depth > 1 {
        return Ok(());
    }

    match value {
        serde_json::Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            for (key, child) in entries {
                push_limited_tag_tokens(markers, key, budget)?;
                push_structured_value_markers_inner(markers, child, depth + 1, budget)?;
                if *budget == 0 {
                    break;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for child in values.iter().take(8) {
                push_structured_value_markers_inner(markers, child, depth + 1, budget)?;
                if *budget == 0 {
                    break;
                }
            }
        }
        serde_json::Value::String(value) => {
            if value.len() <= 64 {
                push_limited_tag_tokens(markers, value, budget)?;
            }
        }
        serde_json::Value::Number(value) => {
            push_limited_tag_tokens(markers, &value.to_string(), budget)?;
        }
        serde_json::Value::Bool(value) => {
            push_limited_tag_tokens(markers, &value.to_string(), budget)?;
        }
        serde_json::Value::Null => {}
    }

    Ok(())
}

fn push_limited_tag_tokens(
    markers: &mut Vec<String>,
    text: &str,
    budget: &mut usize,
) -> Result<()> {
    for token in tokenize_keywords(text) {
        if *budget == 0 {
            break;
        }
        push_marker(markers, format!("tag:{token}"))?;
        *budget -= 1;
    }
    Ok(())
}

fn stopwords() -> HashSet<&'static str> {
    [
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "for", "from", "has", "have",
        "how", "i", "in", "is", "it", "of", "on", "or", "should", "that", "the", "this", "to",
        "was", "what", "when", "where", "which", "who", "why", "with", "you", "your",
    ]
    .into_iter()
    .collect()
}
