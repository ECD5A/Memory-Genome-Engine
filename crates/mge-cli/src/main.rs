use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use mge_core::{
    CellId, CompressionKind, IndexKind, InitOptions, MemoryEngine, MemoryKind, MemorySource,
    MemoryStatus, MemoryValue, PageClustererKind, PageCodecKind, RecallRequest, RememberRequest,
    SensitivityLevel, TrustLevel, DEFAULT_STORE_DIR,
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
        #[arg(long, default_value = "json")]
        page_codec: String,

        #[arg(long, default_value = "none")]
        compression: String,

        #[arg(long, default_value = "scope_kind")]
        page_clusterer: String,

        #[arg(long, default_value = "exact_marker_page")]
        index_kind: String,
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
    },
    Recall {
        query: String,

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
    },
    Seal,
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    Inspect,
    Validate {
        #[arg(long)]
        json: bool,
    },
    Stats {
        #[arg(long)]
        json: bool,
    },
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommands {
    Show {
        #[arg(long)]
        json: bool,
    },
    Set {
        #[arg(long)]
        page_codec: Option<String>,

        #[arg(long)]
        compression: Option<String>,

        #[arg(long)]
        page_clusterer: Option<String>,

        #[arg(long)]
        index_kind: Option<String>,

        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            page_codec,
            compression,
            page_clusterer,
            index_kind,
        } => {
            let options = InitOptions {
                page_codec: PageCodecKind::from_str(&page_codec)?,
                compression: CompressionKind::from_str(&compression)?,
                index_kind: IndexKind::from_str(&index_kind)?,
                page_clusterer: PageClustererKind::from_str(&page_clusterer)?,
            };
            let engine = MemoryEngine::init_with_options(&cli.store, options)
                .with_context(|| format!("failed to initialize {}", cli.store.display()))?;
            println!(
                "Initialized Memory Genome store at {} (page_codec={}, compression={}, page_clusterer={}, index_kind={})",
                engine.root().display(),
                options.page_codec,
                options.compression,
                options.page_clusterer,
                options.index_kind
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
        } => {
            let mut engine = open_engine(&cli.store)?;
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
            max_items,
            markers,
            scope,
            kind,
            json,
            include_deprecated,
            include_secret_references,
        } => {
            let engine = open_engine(&cli.store)?;
            let mut request = RecallRequest::new(query);
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
        Commands::Seal => {
            let mut engine = open_engine(&cli.store)?;
            let report = engine.seal()?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::Config { command } => match command {
            ConfigCommands::Show { json } => {
                let engine = open_engine(&cli.store)?;
                let config = engine.storage_config();
                if json {
                    println!("{}", serde_json::to_string_pretty(&config)?);
                } else {
                    println!("page codec: {}", config.page_codec);
                    println!("compression: {}", config.compression);
                    println!("index kind: {}", config.index_kind);
                    println!("page clusterer: {}", config.page_clusterer);
                }
            }
            ConfigCommands::Set {
                page_codec,
                compression,
                page_clusterer,
                index_kind,
                json,
            } => {
                if page_codec.is_none()
                    && compression.is_none()
                    && page_clusterer.is_none()
                    && index_kind.is_none()
                {
                    bail!("config set requires --page-codec, --compression, --page-clusterer, or --index-kind");
                }

                let mut engine = open_engine(&cli.store)?;
                let report = engine.update_storage_config(mge_core::StorageConfigUpdate {
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
                })?;

                if json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "storage config: page_codec {} -> {}, compression {} -> {}, index_kind {} -> {}, page_clusterer {} -> {}",
                        report.previous.page_codec,
                        report.current.page_codec,
                        report.previous.compression,
                        report.current.compression,
                        report.previous.index_kind,
                        report.current.index_kind,
                        report.previous.page_clusterer,
                        report.current.page_clusterer
                    );
                    println!(
                        "existing sealed pages unchanged: {}",
                        report.existing_pages_unchanged
                    );
                }
            }
        },
        Commands::Inspect => {
            let engine = open_engine(&cli.store)?;
            println!("{}", serde_json::to_string_pretty(&engine.inspect()?)?);
        }
        Commands::Validate { json } => {
            let engine = open_engine(&cli.store)?;
            let report = engine.validate()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", report.to_human_text());
            }
            if !report.ok {
                bail!("store validation failed");
            }
        }
        Commands::Stats { json } => {
            let engine = open_engine(&cli.store)?;
            let stats = engine.stats()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                print!("{}", stats.to_human_text());
            }
        }
        Commands::Export { format } => {
            if format != "json" {
                bail!("only --format json is supported in v0.1");
            }
            let engine = open_engine(&cli.store)?;
            println!("{}", serde_json::to_string_pretty(&engine.export_json()?)?);
        }
    }

    Ok(())
}

fn open_engine(store: &PathBuf) -> Result<MemoryEngine> {
    MemoryEngine::open_at(store).with_context(|| {
        format!(
            "failed to open {}; run `mge init` first or pass --store",
            store.display()
        )
    })
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
