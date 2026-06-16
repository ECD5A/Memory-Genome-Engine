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

mod app_service;
mod tui;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use app_service::{doctor_report, AppService};
use clap::{Parser, Subcommand};
use mge_core::{
    CellId, CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemorySource, MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallMode,
    RecallRequest, RememberRequest, SecurityMode, SensitivityLevel, TrustLevel, DEFAULT_STORE_DIR,
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
    Mark {
        cell_id: CellId,

        #[arg(long)]
        status: String,

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
    Import {
        #[command(subcommand)]
        command: ImportCommands,
    },
    Setup {
        #[arg(long)]
        encrypted: bool,

        #[arg(long)]
        passphrase_env: Option<String>,
    },
    Tui {
        #[arg(long)]
        passphrase_env: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ImportCommands {
    Markdown {
        path: PathBuf,

        #[arg(long, default_value = "import")]
        scope: String,

        #[arg(long, default_value = "temporary_note")]
        kind: String,

        #[arg(long, default_value = "agent_inferred")]
        trust: String,

        #[arg(long, default_value = "private")]
        sensitivity: String,

        #[arg(long = "marker")]
        markers: Vec<String>,

        #[arg(long)]
        json: bool,

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
        Commands::Mark {
            cell_id,
            status,
            json,
            passphrase_env,
        } => {
            let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let status = parse_maintenance_status(&status)?;
            let report = engine.set_status_override(cell_id, status)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if report.override_cleared {
                println!(
                    "cell {} status override cleared; effective status is {}",
                    report.cell_id, report.effective_status
                );
            } else {
                println!(
                    "cell {} effective status set to {} (pages rewritten: {})",
                    report.cell_id, report.effective_status, report.pages_rewritten
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
        Commands::Import { command } => match command {
            ImportCommands::Markdown {
                path,
                scope,
                kind,
                trust,
                sensitivity,
                markers,
                json,
                passphrase_env,
            } => {
                let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
                let report = import_markdown(
                    &mut engine,
                    &path,
                    &scope,
                    &kind,
                    &trust,
                    &sensitivity,
                    &markers,
                )?;
                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "Imported Markdown: files={}, cells={}, skipped={}",
                        report.files_imported, report.cells_imported, report.files_skipped
                    );
                    if !report.skipped.is_empty() {
                        for skipped in &report.skipped {
                            println!("- skipped {}: {}", skipped.path.display(), skipped.reason);
                        }
                    }
                }
            }
        },
        Commands::Setup {
            encrypted,
            passphrase_env,
        } => {
            let service = AppService::new(cli.store.clone(), passphrase_env);
            let report = service.setup_fast(encrypted)?;
            print!("{}", report.to_human_text());
        }
        Commands::Tui { passphrase_env } => {
            tui::run(tui::TuiOptions {
                store: cli.store.clone(),
                passphrase_env,
            })?;
        }
    }

    Ok(())
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

fn parse_maintenance_status(raw: &str) -> Result<MemoryStatus> {
    let status = MemoryStatus::from_str(raw)?;
    if matches!(
        status,
        MemoryStatus::Active
            | MemoryStatus::Deprecated
            | MemoryStatus::Rejected
            | MemoryStatus::Superseded
    ) {
        Ok(status)
    } else {
        bail!(
            "mark supports --status deprecated|rejected|superseded, or active to clear an override"
        )
    }
}

#[derive(Debug, Serialize)]
struct MarkdownImportReport {
    files_imported: usize,
    cells_imported: usize,
    files_skipped: usize,
    skipped: Vec<MarkdownImportSkip>,
    runtime_storage: &'static str,
}

#[derive(Debug, Serialize)]
struct MarkdownImportSkip {
    path: PathBuf,
    reason: String,
}

#[derive(Debug)]
struct MarkdownSection {
    subject: Option<String>,
    content: String,
}

fn import_markdown(
    engine: &mut MemoryEngine,
    path: &Path,
    scope: &str,
    kind: &str,
    trust: &str,
    sensitivity: &str,
    markers: &[String],
) -> Result<MarkdownImportReport> {
    let parsed_kind = MemoryKind::from_str(kind)?;
    let parsed_trust = TrustLevel::from_str(trust)?;
    let parsed_sensitivity = SensitivityLevel::from_str(sensitivity)?;
    let mut report = MarkdownImportReport {
        files_imported: 0,
        cells_imported: 0,
        files_skipped: 0,
        skipped: Vec::new(),
        runtime_storage: "binary",
    };

    let files = collect_markdown_files(path, &mut report)?;
    if files.is_empty() {
        bail!("no Markdown files found at {}", path.display());
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
            request.markers = markdown_import_markers(markers, &file);
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
                .map(|stem| stem.to_string())
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
}
