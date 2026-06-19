use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use mge_core::binary::{self, FileKind};
use mge_core::security::unlock_security_metadata;
use mge_core::store::Manifest;
use mge_core::{
    CompressionKind, IndexKind, InitOptions, MemoryEngine, MemoryKind, MemorySource, MemoryStatus,
    MemoryValue, PageClustererKind, PageCodecKind, RecallMode, RecallRequest, RememberRequest,
    SecurityMode, SensitivityLevel, StoreStats, TrustLevel, ValidationReport,
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

    pub fn import_markdown(&self, input: MarkdownImportInput) -> Result<MarkdownImportReport> {
        let mut engine = self.open_engine()?;
        import_markdown_into_engine(&mut engine, input)
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

#[derive(Clone, Debug)]
pub struct MarkdownImportInput {
    pub path: PathBuf,
    pub scope: String,
    pub kind: String,
    pub trust: String,
    pub sensitivity: String,
    pub markers: Vec<String>,
}

impl Default for MarkdownImportInput {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            scope: "import".to_string(),
            kind: "temporary_note".to_string(),
            trust: "agent_inferred".to_string(),
            sensitivity: "private".to_string(),
            markers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkdownImportReport {
    pub files_imported: usize,
    pub cells_imported: usize,
    pub files_skipped: usize,
    pub skipped: Vec<MarkdownImportSkip>,
    pub runtime_storage: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarkdownImportSkip {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug)]
struct MarkdownSection {
    subject: Option<String>,
    content: String,
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

fn import_markdown_into_engine(
    engine: &mut MemoryEngine,
    input: MarkdownImportInput,
) -> Result<MarkdownImportReport> {
    if input.path.as_os_str().is_empty() {
        bail!("Markdown import path is required");
    }
    let scope = input.scope.trim();
    if scope.is_empty() {
        bail!("Markdown import scope is required");
    }
    let parsed_kind = MemoryKind::from_str(&input.kind)?;
    let parsed_trust = TrustLevel::from_str(&input.trust)?;
    let parsed_sensitivity = SensitivityLevel::from_str(&input.sensitivity)?;
    let mut report = MarkdownImportReport {
        files_imported: 0,
        cells_imported: 0,
        files_skipped: 0,
        skipped: Vec::new(),
        runtime_storage: "binary",
    };

    let files = collect_markdown_files(&input.path, &mut report)?;
    if files.is_empty() {
        bail!("no Markdown files found at {}", input.path.display());
    }

    for file in files {
        let content = fs::read_to_string(&file)
            .with_context(|| format!("failed to read Markdown file {}", file.display()))?;
        let sections = parse_markdown_sections(&content, &file);
        if sections.is_empty() {
            report.files_skipped += 1;
            report.skipped.push(MarkdownImportSkip {
                path: file,
                reason: "empty Markdown file".to_string(),
            });
            continue;
        }

        for section in sections {
            let mut request = RememberRequest::new(parsed_kind, MemoryValue::Text(section.content));
            request.subject = section.subject;
            request.scope = scope.to_string();
            request.status = MemoryStatus::Active;
            request.trust = parsed_trust;
            request.sensitivity = parsed_sensitivity;
            request.markers = markdown_import_markers(&input.markers, &file);
            request.source = Some(MemorySource {
                source_type: "markdown_import".to_string(),
                reference: file.display().to_string(),
            });
            engine.remember(request)?;
            report.cells_imported += 1;
        }
        report.files_imported += 1;
    }

    Ok(report)
}

fn collect_markdown_files(path: &Path, report: &mut MarkdownImportReport) -> Result<Vec<PathBuf>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("path not found: {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("refusing to import symlink path: {}", path.display());
    }
    if metadata.is_file() {
        if is_markdown_path(path) {
            return Ok(vec![path.to_path_buf()]);
        }
        bail!("input file is not Markdown: {}", path.display());
    }
    if !metadata.is_dir() {
        bail!(
            "input path is neither file nor directory: {}",
            path.display()
        );
    }

    let mut files = Vec::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in
            fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let entry_path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                report.files_skipped += 1;
                report.skipped.push(MarkdownImportSkip {
                    path: entry_path,
                    reason: "symlink skipped".to_string(),
                });
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry_path);
            } else if file_type.is_file() && is_markdown_path(&entry_path) {
                files.push(entry_path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown"
            )
        })
        .unwrap_or(false)
}

fn parse_markdown_sections(content: &str, path: &Path) -> Vec<MarkdownSection> {
    let content = content.trim_start_matches('\u{feff}');
    let mut sections = Vec::new();
    let mut current_subject: Option<String> = None;
    let mut current_lines = Vec::new();

    for line in content.lines() {
        if let Some(heading) = markdown_heading(line) {
            push_markdown_section(
                &mut sections,
                current_subject.take(),
                &mut current_lines,
                path,
            );
            current_subject = Some(heading);
        } else {
            current_lines.push(line.to_string());
        }
    }
    push_markdown_section(&mut sections, current_subject, &mut current_lines, path);
    sections
}

fn markdown_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let heading = trimmed[hashes..].trim();
    if heading.is_empty() {
        None
    } else {
        Some(heading.to_string())
    }
}

fn push_markdown_section(
    sections: &mut Vec<MarkdownSection>,
    subject: Option<String>,
    lines: &mut Vec<String>,
    path: &Path,
) {
    let body = lines.join("\n").trim().to_string();
    lines.clear();
    let content = if body.is_empty() {
        subject.clone().unwrap_or_default()
    } else {
        body
    };
    if content.is_empty() {
        return;
    }
    sections.push(MarkdownSection {
        subject: subject.or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        }),
        content,
    });
}

fn markdown_import_markers(base: &[String], path: &Path) -> Vec<String> {
    let mut markers = base.to_vec();
    markers.push("import:markdown".to_string());
    markers.push("source:markdown".to_string());
    if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
        markers.push(format!("file:{stem}"));
    }
    markers
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

    #[test]
    fn markdown_sections_tolerate_utf8_bom() {
        let sections = parse_markdown_sections(
            "\u{feff}# Imported Note\n\nImported body.",
            Path::new("note.md"),
        );

        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].subject.as_deref(), Some("Imported Note"));
        assert_eq!(sections[0].content, "Imported body.");
    }

    #[test]
    fn app_service_imports_markdown_into_binary_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".memory-genome");
        let markdown = dir.path().join("notes.md");
        fs::write(
            &markdown,
            "# First\n\nAlpha memory.\n\n# Second\n\nBeta memory.",
        )
        .unwrap();
        let service = AppService::new(&store, None);
        service.setup_fast(false).unwrap();

        let report = service
            .import_markdown(MarkdownImportInput {
                path: markdown,
                scope: "project:import-test".to_string(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(report.files_imported, 1);
        assert_eq!(report.cells_imported, 2);
        assert_eq!(report.runtime_storage, "binary");
        assert_eq!(service.stats().unwrap().hot_cells, 2);
        assert!(store.join("hot").join("hot.mgl").is_file());
        assert!(!store.join("hot").join("hot.jsonl").exists());
    }
}
