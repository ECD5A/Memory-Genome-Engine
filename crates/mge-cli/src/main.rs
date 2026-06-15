use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use mge_core::binary::{self, FileKind};
use mge_core::security::unlock_security_metadata;
use mge_core::store::Manifest;
use mge_core::{
    CellId, CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemorySource, MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode,
    RecallRequest, RememberRequest, SecurityMode, SensitivityLevel, TrustLevel, ValidationReport,
    DEFAULT_STORE_DIR,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "mge", version, about = "Memory Genome Engine CLI")]
struct Cli {
    #[arg(long, default_value = DEFAULT_STORE_DIR)]
    store: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        #[arg(long, default_value = "debug")]
        profile: String,

        #[arg(long)]
        page_codec: Option<String>,

        #[arg(long)]
        compression: Option<String>,

        #[arg(long)]
        page_clusterer: Option<String>,

        #[arg(long)]
        index_kind: Option<String>,

        #[arg(long)]
        durability: Option<String>,

        #[arg(long)]
        encrypted: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Remember {
        text: Option<String>,

        #[arg(long, default_value = "temporary_note")]
        kind: String,

        #[arg(long)]
        subject: Option<String>,

        #[arg(long)]
        value: Option<String>,

        #[arg(long = "json-value")]
        json_value: Option<String>,

        #[arg(long = "reference-value")]
        reference_value: Option<String>,

        #[arg(long = "timestamp-value")]
        timestamp_value: Option<String>,

        #[arg(long, default_value = "global")]
        scope: String,

        #[arg(long, default_value = "agent_inferred")]
        trust: String,

        #[arg(long, default_value = "active")]
        status: String,

        #[arg(long, default_value = "private")]
        sensitivity: String,

        #[arg(long = "marker")]
        markers: Vec<String>,

        #[arg(long = "source-type")]
        source_type: Option<String>,

        #[arg(long = "source-ref")]
        source_ref: Option<String>,

        #[arg(long = "link")]
        links: Vec<CellId>,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Recall {
        query: Option<String>,

        #[arg(long, default_value = "focused")]
        mode: String,

        #[arg(long, default_value_t = 5)]
        max_items: usize,

        #[arg(long = "marker")]
        markers: Vec<String>,

        #[arg(long)]
        scope: Option<String>,

        #[arg(long)]
        kind: Option<String>,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        include_deprecated: bool,

        #[arg(long)]
        include_secret_references: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Seal {
        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Checkpoint {
        #[arg(long)]
        json: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Inspect {
        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Validate {
        #[arg(long)]
        deep: bool,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    RebuildIndexes {
        #[arg(long)]
        json: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Stats {
        #[arg(long)]
        json: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Doctor {
        #[arg(long)]
        store: Option<PathBuf>,

        #[arg(long)]
        deep: bool,

        #[arg(long)]
        json: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Export {
        #[arg(long, default_value = "markdown")]
        format: String,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
    Security {
        #[arg(long)]
        json: bool,
    },
    Set {
        key: Option<String>,

        value: Option<String>,

        #[arg(long)]
        page_codec: Option<String>,

        #[arg(long)]
        compression: Option<String>,

        #[arg(long)]
        page_clusterer: Option<String>,

        #[arg(long)]
        index_kind: Option<String>,

        #[arg(long)]
        durability: Option<String>,

        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            profile,
            page_codec,
            compression,
            page_clusterer,
            index_kind,
            durability,
            encrypted,
            passphrase_env,
        } => {
            let options = init_options_from_args(
                &profile,
                page_codec.as_deref(),
                compression.as_deref(),
                page_clusterer.as_deref(),
                index_kind.as_deref(),
                durability.as_deref(),
                encrypted,
            )?;
            let passphrase = passphrase_from_env(passphrase_env.as_deref())?;
            let engine = MemoryEngine::init_with_options_and_passphrase(
                &cli.store,
                options,
                passphrase.as_deref(),
            )
            .with_context(|| format!("failed to initialize {}", cli.store.display()))?;
            let security = engine.security_config();
            println!(
                "Initialized Memory Genome store at {} (profile={}, security_mode={}, page_codec={}, compression={}, page_clusterer={}, index_kind={}, durability={})",
                engine.root().display(),
                profile,
                security.mode,
                options.page_codec,
                options.compression,
                options.page_clusterer,
                options.index_kind,
                options.durability
            );
        }
        Commands::Remember {
            text,
            kind,
            subject,
            value,
            json_value,
            reference_value,
            timestamp_value,
            scope,
            trust,
            status,
            sensitivity,
            markers,
            source_type,
            source_ref,
            links,
            passphrase_env,
        } => {
            let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let parsed_kind = MemoryKind::from_str(&kind)?;
            let parsed_trust = TrustLevel::from_str(&trust)?;
            let parsed_status = MemoryStatus::from_str(&status)?;
            let parsed_sensitivity = SensitivityLevel::from_str(&sensitivity)?;
            let memory_value =
                parse_memory_value(text, value, json_value, reference_value, timestamp_value)?;

            let mut request = RememberRequest::new(parsed_kind, memory_value);
            request.subject = subject;
            request.scope = scope;
            request.trust = parsed_trust;
            request.status = parsed_status;
            request.sensitivity = parsed_sensitivity;
            request.markers = markers;
            request.source = parse_memory_source(source_type, source_ref)?;
            request.links = links;

            let cell = engine.remember(request)?;
            println!("Remembered cell {}", cell.id);
        }
        Commands::Recall {
            query,
            mode,
            max_items,
            markers,
            scope,
            kind,
            json,
            include_deprecated,
            include_secret_references,
            passphrase_env,
        } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let parsed_mode = RecallMode::from_str(&mode)?;
            let query = match (query, parsed_mode) {
                (Some(query), _) => query,
                (None, RecallMode::FullScope) => String::new(),
                (None, RecallMode::Focused | RecallMode::Broad) => {
                    bail!("recall query is required unless --mode full-scope is used")
                }
            };
            if parsed_mode == RecallMode::FullScope && scope.is_none() {
                bail!("full-scope recall requires --scope <scope>");
            }
            let mut request = RecallRequest::new(query);
            request.mode = parsed_mode;
            request.max_items = max_items;
            request.markers = markers;
            request.scope = scope;
            request.kind = kind.as_deref().map(MemoryKind::from_str).transpose()?;
            request.include_deprecated = include_deprecated;
            request.include_secret_references = include_secret_references;

            let packet = engine.recall(request)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&packet)?);
            } else {
                print!("{}", packet.to_prompt_text());
            }
        }
        Commands::Seal { passphrase_env } => {
            let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let report = engine.seal()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::Checkpoint {
            json,
            passphrase_env,
        } => {
            let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let report = engine.checkpoint()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "checkpoint: hot_cells={}, snapshot={}, hot_log_offset={}, durability={}",
                    report.hot_cells,
                    report.snapshot_path.display(),
                    report.hot_log_offset,
                    report.durability
                );
            }
        }
        Commands::Config { command } => match command {
            ConfigCommands::Show { json } => {
                let engine = open_engine(&cli.store, None)?;
                let config = engine.storage_config();
                if json {
                    println!("{}", serde_json::to_string_pretty(&config)?);
                } else {
                    println!("page codec: {}", config.page_codec);
                    println!("compression: {}", config.compression);
                    println!("index kind: {}", config.index_kind);
                    println!("page clusterer: {}", config.page_clusterer);
                    println!("durability: {}", config.durability);
                }
            }
            ConfigCommands::Security { json } => {
                let security = MemoryEngine::security_config_at(&cli.store)?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&security)?);
                } else {
                    println!("security mode: {}", security.mode);
                    println!("payload encryption: {}", security.payload_encryption);
                    println!(
                        "session unlock required: {}",
                        security.session_unlock_required
                    );
                    println!("metadata plaintext: {}", security.metadata_plaintext);
                    println!("implementation status: {}", security.implementation_status);
                }
            }
            ConfigCommands::Set {
                key,
                value,
                page_codec,
                compression,
                page_clusterer,
                index_kind,
                durability,
                json,
            } => {
                if page_codec.is_none()
                    && compression.is_none()
                    && page_clusterer.is_none()
                    && index_kind.is_none()
                    && durability.is_none()
                    && key.is_none()
                {
                    bail!("config set requires durability <fast|balanced|safe>, --durability, --page-codec, --compression, --page-clusterer, or --index-kind");
                }

                let mut engine = open_engine(&cli.store, None)?;
                let mut update = mge_core::StorageConfigUpdate {
                    page_codec: page_codec
                        .as_deref()
                        .map(PageCodecKind::from_str)
                        .transpose()?,
                    compression: compression
                        .as_deref()
                        .map(CompressionKind::from_str)
                        .transpose()?,
                    index_kind: index_kind.as_deref().map(IndexKind::from_str).transpose()?,
                    page_clusterer: page_clusterer
                        .as_deref()
                        .map(PageClustererKind::from_str)
                        .transpose()?,
                    durability: durability
                        .as_deref()
                        .map(DurabilityPolicy::from_str)
                        .transpose()?,
                };
                apply_positional_config_update(&mut update, key, value)?;
                let report = engine.update_storage_config(update)?;

                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "storage config: page_codec {} -> {}, compression {} -> {}, index_kind {} -> {}, page_clusterer {} -> {}, durability {} -> {}",
                        report.previous.page_codec,
                        report.current.page_codec,
                        report.previous.compression,
                        report.current.compression,
                        report.previous.index_kind,
                        report.current.index_kind,
                        report.previous.page_clusterer,
                        report.current.page_clusterer,
                        report.previous.durability,
                        report.current.durability
                    );
                    println!(
                        "existing sealed pages unchanged: {}",
                        report.existing_pages_unchanged
                    );
                }
            }
        },
        Commands::Inspect { passphrase_env } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&engine.inspect()?)?);
        }
        Commands::Validate {
            deep,
            json,
            passphrase_env,
        } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let report = if deep {
                engine.validate_deep()?
            } else {
                engine.validate()?
            };
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_human_text());
            }
            if !report.ok {
                bail!("store validation failed");
            }
        }
        Commands::RebuildIndexes {
            json,
            passphrase_env,
        } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let report = engine.rebuild_catalog_and_indexes()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_human_text());
            }
        }
        Commands::Stats {
            json,
            passphrase_env,
        } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let stats = engine.stats()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                print!("{}", stats.to_human_text());
            }
        }
        Commands::Doctor {
            store,
            deep,
            json,
            passphrase_env,
        } => {
            let store = store.unwrap_or_else(|| cli.store.clone());
            let report = doctor_report(&store, deep, passphrase_env.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_human_text());
            }
            if !report.ok {
                bail!("doctor found store issues");
            }
        }
        Commands::Export {
            format,
            passphrase_env,
        } => {
            let engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            match format.as_str() {
                "markdown" | "md" => {
                    let path = engine.export_markdown_to_default_path()?;
                    println!("Exported Markdown memory to {}", path.display());
                }
                "json" | "debug-json" => {
                    println!("{}", serde_json::to_string_pretty(&engine.export_json()?)?);
                }
                other => bail!("unsupported export format: {other}; supported: markdown, json"),
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Serialize)]
struct DoctorFileStatus {
    path: String,
    exists: bool,
    is_dir: bool,
    required: bool,
    size_bytes: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    store_path: String,
    exists: bool,
    initialized: bool,
    manifest_readable: bool,
    manifest_error: Option<String>,
    store_version: Option<u32>,
    security_mode: Option<String>,
    payload_encryption: Option<bool>,
    session_unlock_required: Option<bool>,
    passphrase_required: bool,
    passphrase_env_provided: bool,
    unlock_status: String,
    key_metadata_present: Option<bool>,
    page_codec: Option<String>,
    compression: Option<String>,
    index_kind: Option<String>,
    durability: Option<String>,
    page_files: usize,
    file_statuses: Vec<DoctorFileStatus>,
    validate_recommendation: String,
    deep_requested: bool,
    deep_validation: Option<ValidationReport>,
    deep_validation_skipped: Option<String>,
    issues: Vec<String>,
    warnings: Vec<String>,
}

impl DoctorReport {
    fn to_human_text(&self) -> String {
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

fn doctor_report(
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
            if passphrase.is_some() {
                unlock_security_metadata(&manifest.security, passphrase.as_deref().unwrap())
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

fn open_engine(store: &PathBuf, passphrase_env: Option<&str>) -> Result<MemoryEngine> {
    let passphrase = passphrase_from_env(passphrase_env)?;
    MemoryEngine::open_at_with_passphrase(store, passphrase.as_deref()).with_context(|| {
        format!(
            "failed to open {}; run `mge init` first or pass --store",
            store.display()
        )
    })
}

fn passphrase_from_env(passphrase_env: Option<&str>) -> Result<Option<String>> {
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

fn init_options_from_args(
    profile: &str,
    page_codec: Option<&str>,
    compression: Option<&str>,
    page_clusterer: Option<&str>,
    index_kind: Option<&str>,
    durability: Option<&str>,
    encrypted: bool,
) -> Result<InitOptions> {
    let mut options = init_options_for_profile(profile)?;
    if encrypted {
        options.security_mode = SecurityMode::Encrypted;
    }
    if let Some(page_codec) = page_codec {
        options.page_codec = PageCodecKind::from_str(page_codec)?;
    }
    if let Some(compression) = compression {
        options.compression = CompressionKind::from_str(compression)?;
    }
    if let Some(page_clusterer) = page_clusterer {
        options.page_clusterer = PageClustererKind::from_str(page_clusterer)?;
    }
    if let Some(index_kind) = index_kind {
        options.index_kind = IndexKind::from_str(index_kind)?;
    }
    if let Some(durability) = durability {
        options.durability = DurabilityPolicy::from_str(durability)?;
    }
    Ok(options)
}

fn init_options_for_profile(profile: &str) -> Result<InitOptions> {
    match profile {
        "debug" | "default" | "compat" => Ok(InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::None,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
            security_mode: SecurityMode::Unencrypted,
        }),
        "fast" | "compact" => Ok(InitOptions {
            page_codec: PageCodecKind::MessagePack,
            compression: CompressionKind::Zstd,
            index_kind: IndexKind::ExactMarkerPage,
            page_clusterer: PageClustererKind::ScopeKind,
            durability: DurabilityPolicy::Balanced,
            security_mode: SecurityMode::Unencrypted,
        }),
        other => bail!("unknown init profile: {other}; supported: debug, fast"),
    }
}

fn apply_positional_config_update(
    update: &mut mge_core::StorageConfigUpdate,
    key: Option<String>,
    value: Option<String>,
) -> Result<()> {
    let Some(key) = key else {
        if value.is_some() {
            bail!("config set positional value requires a key");
        }
        return Ok(());
    };
    let Some(value) = value else {
        bail!("config set {key} requires a value");
    };

    match key.replace('-', "_").as_str() {
        "durability" => update.durability = Some(DurabilityPolicy::from_str(&value)?),
        other => bail!(
            "unsupported positional config key: {other}; supported positional key: durability"
        ),
    }
    Ok(())
}

fn parse_memory_value(
    text: Option<String>,
    value: Option<String>,
    json_value: Option<String>,
    reference_value: Option<String>,
    timestamp_value: Option<String>,
) -> Result<MemoryValue> {
    let provided_count = [
        text.is_some(),
        value.is_some(),
        json_value.is_some(),
        reference_value.is_some(),
        timestamp_value.is_some(),
    ]
    .into_iter()
    .filter(|provided| *provided)
    .count();

    if provided_count > 1 {
        bail!(
            "remember accepts exactly one of a text argument, --value, --json-value, --reference-value, or --timestamp-value"
        );
    }

    if let Some(text) = text {
        return Ok(MemoryValue::Text(text));
    }
    if let Some(value) = value {
        return Ok(parse_scalar_value(&value));
    }
    if let Some(json_value) = json_value {
        let parsed = serde_json::from_str(&json_value)
            .with_context(|| "failed to parse --json-value as JSON")?;
        return Ok(MemoryValue::Structured(parsed));
    }
    if let Some(reference_value) = reference_value {
        return Ok(MemoryValue::Reference(reference_value));
    }
    if let Some(timestamp_value) = timestamp_value {
        let parsed = timestamp_value
            .parse::<i64>()
            .with_context(|| "failed to parse --timestamp-value as unix timestamp seconds")?;
        return Ok(MemoryValue::Timestamp(parsed));
    }

    bail!(
        "remember requires a text argument, --value, --json-value, --reference-value, or --timestamp-value"
    )
}

fn parse_scalar_value(raw: &str) -> MemoryValue {
    if let Ok(value) = raw.parse::<bool>() {
        MemoryValue::Boolean(value)
    } else if let Ok(value) = raw.parse::<f64>() {
        MemoryValue::Number(value)
    } else {
        MemoryValue::Symbol(raw.to_string())
    }
}

fn parse_memory_source(
    source_type: Option<String>,
    source_ref: Option<String>,
) -> Result<Option<MemorySource>> {
    match (source_type, source_ref) {
        (Some(source_type), Some(reference)) => Ok(Some(MemorySource {
            source_type,
            reference,
        })),
        (None, None) => Ok(None),
        _ => bail!("remember source requires both --source-type and --source-ref"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_fast_profile_uses_compact_storage_defaults() {
        let options = init_options_from_args("fast", None, None, None, None, None, false).unwrap();

        assert_eq!(options.page_codec, PageCodecKind::MessagePack);
        assert_eq!(options.compression, CompressionKind::Zstd);
        assert_eq!(options.index_kind, IndexKind::ExactMarkerPage);
        assert_eq!(options.page_clusterer, PageClustererKind::ScopeKind);
        assert_eq!(options.durability, DurabilityPolicy::Balanced);
    }

    #[test]
    fn init_profile_allows_explicit_overrides() {
        let options = init_options_from_args(
            "fast",
            Some("messagepack"),
            Some("none"),
            Some("marker_overlap"),
            Some("binary_fuse_page"),
            Some("safe"),
            true,
        )
        .unwrap();

        assert_eq!(options.page_codec, PageCodecKind::MessagePack);
        assert_eq!(options.compression, CompressionKind::None);
        assert_eq!(options.index_kind, IndexKind::BinaryFusePage);
        assert_eq!(options.page_clusterer, PageClustererKind::MarkerOverlap);
        assert_eq!(options.durability, DurabilityPolicy::Safe);
        assert_eq!(options.security_mode, SecurityMode::Encrypted);
    }

    #[test]
    fn init_profile_rejects_unknown_profile() {
        let err =
            init_options_from_args("unknown", None, None, None, None, None, false).unwrap_err();

        assert!(err.to_string().contains("unknown init profile"));
    }

    #[test]
    fn positional_config_update_accepts_durability() {
        let mut update = mge_core::StorageConfigUpdate::default();

        apply_positional_config_update(
            &mut update,
            Some("durability".to_string()),
            Some("safe".to_string()),
        )
        .unwrap();

        assert_eq!(update.durability, Some(DurabilityPolicy::Safe));
    }

    #[test]
    fn parse_text_memory_value() {
        let value =
            parse_memory_value(Some("plain memory".to_string()), None, None, None, None).unwrap();

        assert_eq!(value, MemoryValue::Text("plain memory".to_string()));
    }

    #[test]
    fn parse_scalar_memory_value() {
        let value = parse_memory_value(None, Some("true".to_string()), None, None, None).unwrap();

        assert_eq!(value, MemoryValue::Boolean(true));
    }

    #[test]
    fn parse_structured_json_memory_value() {
        let value = parse_memory_value(
            None,
            None,
            Some(r#"{"answer_style":"concise","max_examples":2}"#.to_string()),
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            value,
            MemoryValue::Structured(serde_json::json!({
                "answer_style": "concise",
                "max_examples": 2
            }))
        );
    }

    #[test]
    fn parse_reference_memory_value() {
        let value = parse_memory_value(
            None,
            None,
            None,
            Some("vault://references/api-key".to_string()),
            None,
        )
        .unwrap();

        assert_eq!(
            value,
            MemoryValue::Reference("vault://references/api-key".to_string())
        );
    }

    #[test]
    fn parse_timestamp_memory_value() {
        let value =
            parse_memory_value(None, None, None, None, Some("1760000000".to_string())).unwrap();

        assert_eq!(value, MemoryValue::Timestamp(1_760_000_000));
    }

    #[test]
    fn parse_timestamp_memory_value_rejects_invalid_input() {
        let err =
            parse_memory_value(None, None, None, None, Some("not-a-time".to_string())).unwrap_err();

        assert!(err.to_string().contains("--timestamp-value"));
    }

    #[test]
    fn parse_memory_value_rejects_ambiguous_inputs() {
        let err = parse_memory_value(
            Some("plain memory".to_string()),
            Some("true".to_string()),
            Some(r#"{"answer_style":"concise"}"#.to_string()),
            None,
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn parse_memory_source_accepts_complete_source() {
        let source =
            parse_memory_source(Some("issue".to_string()), Some("MGE-1".to_string())).unwrap();

        assert_eq!(
            source,
            Some(MemorySource {
                source_type: "issue".to_string(),
                reference: "MGE-1".to_string()
            })
        );
    }

    #[test]
    fn parse_memory_source_rejects_partial_source() {
        let err = parse_memory_source(Some("issue".to_string()), None).unwrap_err();

        assert!(err.to_string().contains("--source-type and --source-ref"));
    }
}
