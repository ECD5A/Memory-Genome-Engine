use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use mge_core::binary::{self, FileKind};
use mge_core::security::unlock_security_metadata;
use mge_core::store::Manifest;
use mge_core::{
    CompressionKind, IndexKind, InitOptions, MemoryEngine, MemoryKind, MemoryStatus, MemoryValue,
    PageClustererKind, PageCodecKind, RecallMode, RecallRequest, RememberRequest, SecurityMode,
    SensitivityLevel, StoreStats, TrustLevel, ValidationReport,
};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct AppService {
    store: PathBuf,
    passphrase_env: Option<String>,
}

impl AppService {
    pub fn new(store: impl Into<PathBuf>, passphrase_env: Option<String>) -> Self {
        Self {
            store: store.into(),
            passphrase_env,
        }
    }

    pub fn store_path(&self) -> &Path {
        &self.store
    }

    pub fn passphrase_env_name(&self) -> Option<&str> {
        self.passphrase_env.as_deref()
    }

    pub fn setup_fast(&self, encrypted: bool) -> Result<SetupReport> {
        if self.store.join("manifest.mgm").exists() {
            return Ok(SetupReport {
                store_path: self.store.clone(),
                already_initialized: true,
                encrypted,
                passphrase_env: self.passphrase_env.clone(),
                profile: "fast".to_string(),
            });
        }

        let passphrase = if encrypted {
            if self.passphrase_env.is_none() {
                bail!("encrypted setup requires --passphrase-env <ENV_VAR_NAME>");
            }
            passphrase_from_env(self.passphrase_env.as_deref())?
        } else {
            None
        };

        MemoryEngine::init_with_options_and_passphrase(
            &self.store,
            InitOptions {
                page_codec: PageCodecKind::MessagePack,
                compression: CompressionKind::Zstd,
                index_kind: IndexKind::ExactMarkerPage,
                page_clusterer: PageClustererKind::ScopeKind,
                durability: Default::default(),
                security_mode: if encrypted {
                    SecurityMode::Encrypted
                } else {
                    SecurityMode::Unencrypted
                },
            },
            passphrase.as_deref(),
        )
        .with_context(|| format!("failed to initialize {}", self.store.display()))?;

        Ok(SetupReport {
            store_path: self.store.clone(),
            already_initialized: false,
            encrypted,
            passphrase_env: self.passphrase_env.clone(),
            profile: "fast".to_string(),
        })
    }

    pub fn stats(&self) -> Result<StoreStats> {
        self.open_engine()?.stats().map_err(Into::into)
    }

    pub fn dashboard(&self) -> DashboardSummary {
        let doctor = self
            .doctor(false)
            .unwrap_or_else(|err| DoctorReport::from_error(&self.store, err.to_string()));
        let stats = if doctor.passphrase_required && !doctor.passphrase_env_provided {
            None
        } else {
            self.stats().ok()
        };
        DashboardSummary { doctor, stats }
    }

    pub fn doctor(&self, deep: bool) -> Result<DoctorReport> {
        doctor_report(&self.store, deep, self.passphrase_env.as_deref())
    }

    pub fn recall(&self, input: RecallInput) -> Result<mge_core::ContextPacket> {
        let engine = self.open_engine()?;
        let mode = input.mode;
        let query = match (input.query.trim(), mode) {
            ("", RecallMode::FullScope) => String::new(),
            ("", RecallMode::Focused | RecallMode::Broad) => {
                bail!("recall query is required unless mode is full-scope")
            }
            (query, _) => query.to_string(),
        };
        if mode == RecallMode::FullScope && input.scope.trim().is_empty() {
            bail!("full-scope recall requires a scope");
        }
        let mut request = RecallRequest::new(query);
        request.mode = mode;
        request.max_items = input.max_items.max(1);
        request.scope = non_empty(input.scope);
        request.markers = parse_marker_list(&input.markers);
        request.kind = non_empty(input.kind)
            .as_deref()
            .map(MemoryKind::from_str)
            .transpose()?;
        engine.recall(request).map_err(Into::into)
    }

    pub fn remember(&self, input: RememberInput) -> Result<u64> {
        let mut engine = self.open_engine()?;
        let content = input.content.trim();
        if content.is_empty() {
            bail!("memory content is required");
        }
        let mut request = RememberRequest::new(
            MemoryKind::from_str(non_empty(input.kind).as_deref().unwrap_or("temporary_note"))?,
            MemoryValue::Text(content.to_string()),
        );
        request.subject = non_empty(input.subject);
        request.scope = non_empty(input.scope).unwrap_or_else(|| "global".to_string());
        request.status =
            MemoryStatus::from_str(non_empty(input.status).as_deref().unwrap_or("active"))?;
        request.trust = TrustLevel::from_str(
            non_empty(input.trust)
                .as_deref()
                .unwrap_or("agent_inferred"),
        )?;
        request.sensitivity = SensitivityLevel::from_str(
            non_empty(input.sensitivity).as_deref().unwrap_or("private"),
        )?;
        request.markers = parse_marker_list(&input.markers);
        let cell = engine.remember(request)?;
        Ok(cell.id)
    }

    pub fn seal(&self) -> Result<mge_core::SealReport> {
        let mut engine = self.open_engine()?;
        engine.seal().map_err(Into::into)
    }

    pub fn checkpoint(&self) -> Result<mge_core::HotCheckpointReport> {
        let mut engine = self.open_engine()?;
        engine.checkpoint().map_err(Into::into)
    }

    pub fn validate_deep(&self) -> Result<ValidationReport> {
        self.open_engine()?.validate_deep().map_err(Into::into)
    }

    pub fn rebuild_indexes(&self) -> Result<mge_core::RebuildIndexesReport> {
        self.open_engine()?
            .rebuild_catalog_and_indexes()
            .map_err(Into::into)
    }

    pub fn export_markdown(&self) -> Result<PathBuf> {
        self.open_engine()?
            .export_markdown_to_default_path()
            .map_err(Into::into)
    }

    pub fn set_index_kind(
        &self,
        index_kind: IndexKind,
    ) -> Result<mge_core::StorageConfigUpdateReport> {
        let mut engine = self.open_engine()?;
        engine
            .update_storage_config(mge_core::StorageConfigUpdate {
                index_kind: Some(index_kind),
                ..Default::default()
            })
            .map_err(Into::into)
    }

    pub fn run_small_index_benchmark(&self) -> Result<IndexBenchmarkReport> {
        run_small_index_benchmark()
    }

    fn open_engine(&self) -> Result<MemoryEngine> {
        let passphrase = passphrase_from_env(self.passphrase_env.as_deref())?;
        MemoryEngine::open_at_with_passphrase(&self.store, passphrase.as_deref())
            .with_context(|| format!("failed to open store {}", self.store.display()))
    }
}

#[derive(Clone, Debug)]
pub struct DashboardSummary {
    pub doctor: DoctorReport,
    pub stats: Option<StoreStats>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SetupReport {
    pub store_path: PathBuf,
    pub already_initialized: bool,
    pub encrypted: bool,
    pub passphrase_env: Option<String>,
    pub profile: String,
}

impl SetupReport {
    pub fn to_human_text(&self) -> String {
        let state = if self.already_initialized {
            "already initialized"
        } else {
            "initialized"
        };
        let mode = if self.encrypted {
            "encrypted"
        } else {
            "unencrypted"
        };
        let mut output = format!(
            "\
Memory Genome setup
store: {}
state: {}
profile: {}
security: {}
",
            self.store_path.display(),
            state,
            self.profile,
            mode
        );
        if self.encrypted {
            output.push_str(&format!(
                "passphrase env: {}\n",
                self.passphrase_env
                    .as_deref()
                    .unwrap_or("<missing; rerun with --passphrase-env>")
            ));
        }
        output.push_str(
            "\
next steps:
- mge tui
- mge remember \"useful memory\" --kind project_fact --scope global
- mge recall \"useful memory\"
- mge-mcp-server for local JSON-RPC agent hosts
",
        );
        output
    }
}

#[derive(Clone, Debug)]
pub struct RecallInput {
    pub query: String,
    pub mode: RecallMode,
    pub max_items: usize,
    pub markers: String,
    pub scope: String,
    pub kind: String,
}

impl Default for RecallInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: RecallMode::Focused,
            max_items: 5,
            markers: String::new(),
            scope: String::new(),
            kind: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RememberInput {
    pub subject: String,
    pub content: String,
    pub markers: String,
    pub scope: String,
    pub kind: String,
    pub status: String,
    pub trust: String,
    pub sensitivity: String,
}

impl Default for RememberInput {
    fn default() -> Self {
        Self {
            subject: String::new(),
            content: String::new(),
            markers: String::new(),
            scope: "global".to_string(),
            kind: "temporary_note".to_string(),
            status: "active".to_string(),
            trust: "agent_inferred".to_string(),
            sensitivity: "private".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct IndexBenchmarkReport {
    pub exact_recall_micros: u64,
    pub binary_fuse_recall_micros: u64,
    pub exact_candidate_pages: usize,
    pub binary_fuse_candidate_pages: usize,
    pub exact_loaded_pages: usize,
    pub binary_fuse_loaded_pages: usize,
    pub exact_cells_scanned: usize,
    pub binary_fuse_cells_scanned: usize,
    pub exact_result_count: usize,
    pub binary_fuse_result_count: usize,
    pub false_positive_pages: usize,
    pub exact_subset_binary_fuse: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorFileStatus {
    pub path: String,
    pub exists: bool,
    pub is_dir: bool,
    pub required: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub store_path: String,
    pub exists: bool,
    pub initialized: bool,
    pub manifest_readable: bool,
    pub manifest_error: Option<String>,
    pub store_version: Option<u32>,
    pub security_mode: Option<String>,
    pub payload_encryption: Option<bool>,
    pub session_unlock_required: Option<bool>,
    pub passphrase_required: bool,
    pub passphrase_env_provided: bool,
    pub unlock_status: String,
    pub key_metadata_present: Option<bool>,
    pub page_codec: Option<String>,
    pub compression: Option<String>,
    pub index_kind: Option<String>,
    pub durability: Option<String>,
    pub page_files: usize,
    pub file_statuses: Vec<DoctorFileStatus>,
    pub validate_recommendation: String,
    pub deep_requested: bool,
    pub deep_validation: Option<ValidationReport>,
    pub deep_validation_skipped: Option<String>,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

impl DoctorReport {
    pub fn from_error(store: &Path, error: String) -> Self {
        Self {
            ok: false,
            store_path: store.display().to_string(),
            exists: store.exists(),
            initialized: false,
            manifest_readable: false,
            manifest_error: Some(error.clone()),
            store_version: None,
            security_mode: None,
            payload_encryption: None,
            session_unlock_required: None,
            passphrase_required: false,
            passphrase_env_provided: false,
            unlock_status: "error".to_string(),
            key_metadata_present: None,
            page_codec: None,
            compression: None,
            index_kind: None,
            durability: None,
            page_files: 0,
            file_statuses: Vec::new(),
            validate_recommendation: "fix store initialization before validation".to_string(),
            deep_requested: false,
            deep_validation: None,
            deep_validation_skipped: None,
            issues: vec![error],
            warnings: Vec::new(),
        }
    }

    pub fn to_human_text(&self) -> String {
        let mut output = String::new();
        output.push_str("Memory Genome doctor\n");
        output.push_str(&format!("store: {}\n", self.store_path));
        output.push_str(&format!("ok: {}\n", self.ok));
        output.push_str(&format!("exists: {}\n", self.exists));
        output.push_str(&format!("initialized: {}\n", self.initialized));
        output.push_str(&format!("manifest readable: {}\n", self.manifest_readable));
        if let Some(version) = self.store_version {
            output.push_str(&format!("store version: {version}\n"));
        }
        if let Some(mode) = &self.security_mode {
            output.push_str(&format!("security mode: {mode}\n"));
        }
        output.push_str(&format!("unlock status: {}\n", self.unlock_status));
        if let Some(index_kind) = &self.index_kind {
            output.push_str(&format!("index kind: {index_kind}\n"));
        }
        if let Some(page_codec) = &self.page_codec {
            output.push_str(&format!("page codec: {page_codec}\n"));
        }
        if let Some(compression) = &self.compression {
            output.push_str(&format!("compression: {compression}\n"));
        }
        output.push_str(&format!("sealed page files: {}\n", self.page_files));
        output.push_str(&format!(
            "validate recommendation: {}\n",
            self.validate_recommendation
        ));
        if let Some(skipped) = &self.deep_validation_skipped {
            output.push_str(&format!("deep validation skipped: {skipped}\n"));
        }
        if let Some(validation) = &self.deep_validation {
            output.push_str(&format!("deep validation ok: {}\n", validation.ok));
            output.push_str(&format!(
                "deep validation errors: {}\n",
                validation.errors.len()
            ));
            output.push_str(&format!(
                "deep validation warnings: {}\n",
                validation.warnings.len()
            ));
        }
        if !self.warnings.is_empty() {
            output.push_str("warnings:\n");
            for warning in &self.warnings {
                output.push_str(&format!("- {warning}\n"));
            }
        }
        if !self.issues.is_empty() {
            output.push_str("issues:\n");
            for issue in &self.issues {
                output.push_str(&format!("- {issue}\n"));
            }
        }
        output
    }
}

pub fn doctor_report(
    store: &Path,
    deep_requested: bool,
    passphrase_env: Option<&str>,
) -> Result<DoctorReport> {
    let root = store.to_path_buf();
    let exists = root.exists();
    let manifest_path = root.join("manifest.mgm");
    let initialized = manifest_path.exists();
    let passphrase = passphrase_from_env(passphrase_env)?;
    let passphrase_env_provided = passphrase_env.is_some();
    let file_statuses = doctor_file_statuses(&root);
    let page_files = count_page_files(&root);
    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    if !exists {
        issues.push("store path does not exist".to_string());
    } else if !root.is_dir() {
        issues.push("store path exists but is not a directory".to_string());
    }
    if exists && !initialized {
        issues.push("manifest.mgm is missing; run `mge init` first".to_string());
    }

    let mut manifest_readable = false;
    let mut manifest_error = None;
    let mut store_version = None;
    let mut security_mode = None;
    let mut payload_encryption = None;
    let mut session_unlock_required = None;
    let mut passphrase_required = false;
    let mut unlock_status = "not_initialized".to_string();
    let mut key_metadata_present = None;
    let mut page_codec = None;
    let mut compression = None;
    let mut index_kind = None;
    let mut durability = None;
    let mut deep_validation = None;
    let mut deep_validation_skipped = None;

    let manifest = if initialized {
        match binary::read_messagepack_file::<Manifest>(&manifest_path, FileKind::Manifest) {
            Ok(manifest) => {
                manifest_readable = true;
                store_version = Some(manifest.version);
                security_mode = Some(manifest.security_mode.to_string());
                page_codec = Some(manifest.page_codec.to_string());
                compression = Some(manifest.compression.to_string());
                index_kind = Some(manifest.index_kind.to_string());
                durability = Some(manifest.durability.to_string());
                Some(manifest)
            }
            Err(err) => {
                let message = err.to_string();
                manifest_error = Some(message.clone());
                issues.push(format!("manifest.mgm is not readable: {message}"));
                None
            }
        }
    } else {
        None
    };

    if let Some(manifest) = &manifest {
        let security = MemoryEngine::security_config_at(&root)?;
        payload_encryption = Some(security.payload_encryption);
        session_unlock_required = Some(security.session_unlock_required);
        passphrase_required = security.session_unlock_required;
        key_metadata_present = Some(security.key_verification_configured);
        if manifest.security_mode.is_encrypted() {
            if let Some(passphrase) = passphrase.as_deref() {
                unlock_security_metadata(&manifest.security, passphrase)
                    .with_context(|| "auth_failed: failed to unlock encrypted store")?;
                unlock_status = "unlocked".to_string();
            } else {
                unlock_status = "locked_passphrase_required".to_string();
                warnings.push(
                    "encrypted store requires --passphrase-env for payload commands or deep doctor"
                        .to_string(),
                );
            }
        } else {
            unlock_status = "not_required".to_string();
        }

        if deep_requested {
            if manifest.security_mode.is_encrypted() && passphrase.is_none() {
                deep_validation_skipped =
                    Some("encrypted store requires --passphrase-env for --deep".to_string());
            } else {
                match MemoryEngine::open_at_read_only_with_passphrase(&root, passphrase.as_deref())
                    .and_then(|engine| engine.validate_deep())
                {
                    Ok(report) => {
                        if !report.ok {
                            issues.push("deep validation reported issues".to_string());
                        }
                        deep_validation = Some(report);
                    }
                    Err(err) => {
                        issues.push(format!("deep validation failed: {err}"));
                    }
                }
            }
        }
    }

    for status in &file_statuses {
        if status.required && !status.exists {
            issues.push(format!("required path missing: {}", status.path));
        }
    }

    let validate_recommendation = if passphrase_required && !passphrase_env_provided {
        "run `mge validate --deep --passphrase-env <ENV>` after unlocking".to_string()
    } else {
        "run `mge validate --deep` for full catalog/index/page validation".to_string()
    };

    let ok = issues.is_empty();
    Ok(DoctorReport {
        ok,
        store_path: root.display().to_string(),
        exists,
        initialized,
        manifest_readable,
        manifest_error,
        store_version,
        security_mode,
        payload_encryption,
        session_unlock_required,
        passphrase_required,
        passphrase_env_provided,
        unlock_status,
        key_metadata_present,
        page_codec,
        compression,
        index_kind,
        durability,
        page_files,
        file_statuses,
        validate_recommendation,
        deep_requested,
        deep_validation,
        deep_validation_skipped,
        issues,
        warnings,
    })
}

pub fn passphrase_from_env(passphrase_env: Option<&str>) -> Result<Option<String>> {
    let Some(name) = passphrase_env else {
        return Ok(None);
    };
    let value = env::var(name).with_context(|| {
        format!("passphrase env var {name} is not set; pass the name with --passphrase-env")
    })?;
    if value.is_empty() {
        bail!("passphrase env var {name} is empty");
    }
    Ok(Some(value))
}

fn run_small_index_benchmark() -> Result<IndexBenchmarkReport> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let root = env::temp_dir().join(format!("mge-tui-index-bench-{run_id}"));
    let exact_root = root.join("exact");
    let binary_root = root.join("binary_fuse");
    let exact = run_index_benchmark_mode(&exact_root, IndexKind::ExactMarkerPage)?;
    let binary = run_index_benchmark_mode(&binary_root, IndexKind::BinaryFusePage)?;
    let _ = fs::remove_dir_all(&root);
    Ok(IndexBenchmarkReport {
        exact_recall_micros: exact.recall_micros,
        binary_fuse_recall_micros: binary.recall_micros,
        exact_candidate_pages: exact.candidate_pages,
        binary_fuse_candidate_pages: binary.candidate_pages,
        exact_loaded_pages: exact.loaded_pages,
        binary_fuse_loaded_pages: binary.loaded_pages,
        exact_cells_scanned: exact.cells_scanned,
        binary_fuse_cells_scanned: binary.cells_scanned,
        exact_result_count: exact.result_count,
        binary_fuse_result_count: binary.result_count,
        false_positive_pages: binary.false_positive_pages,
        exact_subset_binary_fuse: exact.candidate_pages <= binary.candidate_pages,
    })
}

#[derive(Clone, Debug)]
struct BenchModeResult {
    recall_micros: u64,
    candidate_pages: usize,
    loaded_pages: usize,
    cells_scanned: usize,
    result_count: usize,
    false_positive_pages: usize,
}

fn run_index_benchmark_mode(root: &Path, index_kind: IndexKind) -> Result<BenchModeResult> {
    let mut engine = MemoryEngine::init_with_options(
        root,
        InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: Default::default(),
            security_mode: SecurityMode::Unencrypted,
        },
    )?;
    for page in 0..12 {
        for cell in 0..8 {
            let group = page % 4;
            let mut request = RememberRequest::new(
                MemoryKind::ProjectFact,
                MemoryValue::Text(format!(
                    "benchmark group {group} page {page} cell {cell} candidate memory"
                )),
            );
            request.scope = format!("bench_scope_{group}");
            request.trust = TrustLevel::ToolObserved;
            request.sensitivity = SensitivityLevel::Private;
            request.markers = vec![format!("bench_group:{group}"), format!("bench_page:{page}")];
            engine.remember(request)?;
        }
    }
    engine.seal()?;
    let mut recall = RecallRequest::new("benchmark group 1 candidate memory");
    recall.mode = RecallMode::Focused;
    recall.max_items = 8;
    recall.markers = vec!["bench_group:1".to_string()];
    let start = Instant::now();
    let packet = engine.recall(recall)?;
    let recall_micros = start.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    Ok(BenchModeResult {
        recall_micros,
        candidate_pages: packet.debug.candidate_pages_returned,
        loaded_pages: packet.debug.loaded_pages,
        cells_scanned: packet.debug.cells_scanned,
        result_count: packet.relevant_memory.len(),
        false_positive_pages: packet.debug.false_positive_candidate_pages,
    })
}

fn doctor_file_statuses(root: &Path) -> Vec<DoctorFileStatus> {
    [
        ("manifest.mgm", true),
        ("dictionary", true),
        ("dictionary/markers.mgd", true),
        ("hot", true),
        ("hot/hot.mgl", true),
        ("hot/snapshot.mgs", false),
        ("pages", true),
        ("indexes", true),
        ("indexes/page_index.mgi", true),
        ("indexes/marker_index.mgi", true),
        ("indexes/fuse_index.mgi", true),
        ("exports", false),
    ]
    .into_iter()
    .map(|(relative, required)| {
        let path = root.join(relative);
        let metadata = fs::metadata(&path).ok();
        DoctorFileStatus {
            path: relative.to_string(),
            exists: metadata.is_some(),
            is_dir: metadata.as_ref().is_some_and(|metadata| metadata.is_dir()),
            required,
            size_bytes: metadata
                .as_ref()
                .filter(|metadata| metadata.is_file())
                .map(|metadata| metadata.len()),
        }
    })
    .collect()
}

fn count_page_files(root: &Path) -> usize {
    let pages_dir = root.join("pages");
    let Ok(entries) = fs::read_dir(pages_dir) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("mgp")
        })
        .count()
}

fn parse_marker_list(input: &str) -> Vec<String> {
    input
        .split([',', '\n'])
        .map(str::trim)
        .filter(|marker| !marker.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn non_empty(input: String) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_list_splits_commas_and_newlines() {
        assert_eq!(
            parse_marker_list("topic:alpha, scope:beta\nkind:note"),
            vec!["topic:alpha", "scope:beta", "kind:note"]
        );
    }

    #[test]
    fn remember_input_defaults_are_core_compatible() {
        let input = RememberInput::default();
        assert_eq!(input.scope, "global");
        assert_eq!(input.kind, "temporary_note");
        assert_eq!(input.status, "active");
    }

    #[test]
    fn setup_fast_initializes_unencrypted_store_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".memory-genome");
        let service = AppService::new(&store, None);

        let report = service.setup_fast(false).unwrap();

        assert!(!report.already_initialized);
        assert!(!report.encrypted);
        assert!(store.join("manifest.mgm").is_file());

        let second = service.setup_fast(false).unwrap();
        assert!(second.already_initialized);
    }

    #[test]
    fn setup_fast_encrypted_requires_passphrase_env_name() {
        let dir = tempfile::tempdir().unwrap();
        let service = AppService::new(dir.path().join(".memory-genome"), None);

        let err = service.setup_fast(true).unwrap_err();

        assert!(err.to_string().contains("--passphrase-env"));
    }
}
