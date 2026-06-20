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
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use app_service::{doctor_report, AppService, MarkdownImportInput};
use clap::{Parser, Subcommand};
use mge_core::{
    CellId, CompressionKind, DurabilityPolicy, IndexKind, InitOptions, MemoryEngine, MemoryKind,
    MemorySource, MemoryStatus, MemoryValue, MgeError, PageClustererKind, PageCodecKind,
    RecallMode, RecallRequest, RememberRequest, SecurityMode, SensitivityLevel,
    SessionChunkOptions, SessionRememberRequest, SessionTurn, TrustLevel, DEFAULT_STORE_DIR,
};

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
    RememberSession {
        #[arg(long = "turn", required = true)]
        turns: Vec<String>,

        #[arg(long = "session-id")]
        session_id: Option<String>,

        #[arg(long, default_value = "project_fact")]
        kind: String,

        #[arg(long)]
        subject: Option<String>,

        #[arg(long, default_value = "global")]
        scope: String,

        #[arg(long, default_value = "tool_observed")]
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

        #[arg(long, default_value_t = 8)]
        max_turns: usize,

        #[arg(long, default_value_t = 4096)]
        max_bytes: usize,

        #[arg(long)]
        json: bool,

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
            let already_initialized = cli.store.join("manifest.mgm").exists();
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
            if already_initialized && encrypted && !security.mode.is_encrypted() {
                bail!(
                    "store {} is already initialized without encryption; init does not migrate existing stores",
                    cli.store.display()
                );
            }
            let storage = engine.storage_config();
            let state = if already_initialized {
                "Memory Genome store already initialized"
            } else {
                "Initialized Memory Genome store"
            };
            let profile = if already_initialized {
                "existing"
            } else {
                profile.as_str()
            };
            println!(
                "{} at {} (profile={}, security_mode={}, page_codec={}, compression={}, page_clusterer={}, index_kind={}, durability={})",
                state,
                engine.root().display(),
                profile,
                security.mode,
                storage.page_codec,
                storage.compression,
                storage.page_clusterer,
                storage.index_kind,
                storage.durability
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
        Commands::RememberSession {
            turns,
            session_id,
            kind,
            subject,
            scope,
            trust,
            status,
            sensitivity,
            markers,
            source_type,
            source_ref,
            links,
            max_turns,
            max_bytes,
            json,
            passphrase_env,
        } => {
            let mut engine = open_engine(&cli.store, passphrase_env.as_deref())?;
            let turns = turns
                .iter()
                .enumerate()
                .map(|(index, turn)| parse_session_turn(index, turn))
                .collect::<Result<Vec<_>>>()?;
            let mut request = SessionRememberRequest::new(turns);
            request.chunk_options = SessionChunkOptions {
                max_turns,
                max_bytes,
            };
            request.session_id = session_id;
            request.kind = MemoryKind::from_str(&kind)?;
            request.subject = subject;
            request.scope = scope;
            request.trust = TrustLevel::from_str(&trust)?;
            request.status = MemoryStatus::from_str(&status)?;
            request.sensitivity = SensitivityLevel::from_str(&sensitivity)?;
            request.markers = markers;
            request.source = parse_memory_source(source_type, source_ref)?;
            request.links = links;
            let report = engine.remember_session(request)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Remembered session: turns={}, chunks={}, cells={}",
                    report.turns,
                    report.chunks,
                    report.cells.len()
                );
            }
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
                let service = AppService::new(cli.store.clone(), passphrase_env);
                let report = service.import_markdown(MarkdownImportInput {
                    path,
                    scope,
                    kind,
                    trust,
                    sensitivity,
                    markers,
                })?;
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
    MemoryEngine::open_at_with_passphrase(store, passphrase.as_deref()).map_err(|err| {
        let context = open_engine_error_context(store, passphrase_env, &err);
        anyhow::Error::new(err).context(context)
    })
}

fn open_engine_error_context(store: &Path, passphrase_env: Option<&str>, err: &MgeError) -> String {
    match err {
        MgeError::AuthenticationFailed(_) => {
            if let Some(name) = passphrase_env {
                format!(
                    "failed to unlock encrypted store {}; check --passphrase-env {name} and the passphrase value it names",
                    store.display()
                )
            } else {
                format!(
                    "failed to unlock encrypted store {}; pass --passphrase-env <ENV> with the correct passphrase",
                    store.display()
                )
            }
        }
        MgeError::StoreLocked(_) => format!(
            "encrypted store {} is locked; pass --passphrase-env <ENV> to unlock payload operations",
            store.display()
        ),
        MgeError::StoreBusy(_) => format!(
            "store {} is already open; close the other MGE process and retry",
            store.display()
        ),
        MgeError::NotInitialized(_) => format!(
            "failed to open {}; run `mge init` first or pass --store",
            store.display()
        ),
        _ => format!("failed to open {}", store.display()),
    }
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

fn parse_session_turn(index: usize, input: &str) -> Result<SessionTurn> {
    let (role, content) = input
        .split_once('=')
        .or_else(|| input.split_once(':'))
        .with_context(|| format!("turn {index} must use ROLE=CONTENT or ROLE:CONTENT format"))?;
    let role = role.trim();
    let content = content.trim();
    if role.is_empty() || content.is_empty() {
        bail!("turn {index} requires non-empty role and content");
    }
    Ok(SessionTurn::new(role, content))
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
    fn open_engine_error_context_distinguishes_encrypted_auth_failures() {
        let store = Path::new(".memory-genome");

        let auth = open_engine_error_context(
            store,
            Some("MGE_PASSPHRASE"),
            &MgeError::AuthenticationFailed("bad key".to_string()),
        );
        assert!(auth.contains("failed to unlock encrypted store"));
        assert!(auth.contains("--passphrase-env MGE_PASSPHRASE"));
        assert!(!auth.contains("run `mge init`"));

        let locked = open_engine_error_context(
            store,
            None,
            &MgeError::StoreLocked("unlock required".to_string()),
        );
        assert!(locked.contains("encrypted store"));
        assert!(locked.contains("--passphrase-env <ENV>"));

        let uninitialized = open_engine_error_context(
            store,
            None,
            &MgeError::NotInitialized("missing manifest".to_string()),
        );
        assert!(uninitialized.contains("run `mge init`"));
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
