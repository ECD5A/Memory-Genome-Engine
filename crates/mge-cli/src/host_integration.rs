use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use clap::ValueEnum;
use serde::Serialize;
use serde_json::{json, Map, Value};

const SERVER_NAME: &str = "memory-genome";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CliServerState {
    Absent,
    Matching,
    Conflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AgentHost {
    Codex,
    #[value(name = "claude-code")]
    ClaudeCode,
    Cursor,
    #[value(name = "generic-mcp")]
    GenericMcp,
}

impl AgentHost {
    fn command_name(self) -> Option<&'static str> {
        match self {
            Self::Codex => Some("codex"),
            Self::ClaudeCode => Some("claude"),
            Self::Cursor | Self::GenericMcp => None,
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GenericMcp => "generic MCP host",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostSetupOptions {
    pub host: AgentHost,
    pub store: PathBuf,
    pub passphrase_env: Option<String>,
    pub mcp_server: Option<PathBuf>,
    pub dry_run: bool,
    pub remove: bool,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostSetupReport {
    pub host: String,
    pub action: String,
    pub changed: bool,
    pub dry_run: bool,
    pub server_name: String,
    pub store_path: PathBuf,
    pub mcp_server: PathBuf,
    pub config_path: Option<PathBuf>,
    pub command: String,
    pub rollback: String,
    pub smoke_ok: bool,
    pub passphrase_env: Option<String>,
    pub notes: Vec<String>,
}

impl HostSetupReport {
    pub fn to_human_text(&self) -> String {
        let mut output = format!(
            "Agent integration\n- host: {}\n- action: {}\n- changed: {}\n- store: {}\n- MCP server: {}\n- smoke: {}\n- command: {}\n- rollback: {}\n",
            self.host,
            self.action,
            self.changed,
            self.store_path.display(),
            self.mcp_server.display(),
            if self.smoke_ok { "OK" } else { "not run" },
            self.command,
            self.rollback
        );
        if let Some(path) = &self.config_path {
            output.push_str(&format!("- config: {}\n", path.display()));
        }
        if let Some(name) = &self.passphrase_env {
            output.push_str(&format!("- passphrase env: {name} (value not persisted)\n"));
        }
        for note in &self.notes {
            output.push_str(&format!("- note: {note}\n"));
        }
        output
    }
}

pub fn configure_host(options: HostSetupOptions) -> Result<HostSetupReport> {
    let store = absolute_path(&options.store)?;
    let mcp_server = if options.remove {
        resolve_mcp_server_for_removal(options.mcp_server.as_deref())?
    } else {
        resolve_mcp_server(options.mcp_server.as_deref())?
    };
    let server_args = server_args(&store, options.passphrase_env.as_deref());

    if !options.remove && !options.dry_run {
        smoke_server(&mcp_server, &server_args)?;
    }

    match options.host {
        AgentHost::Codex => configure_cli_host(options, store, mcp_server, server_args, false),
        AgentHost::ClaudeCode => configure_cli_host(options, store, mcp_server, server_args, true),
        AgentHost::Cursor => configure_cursor(options, store, mcp_server, server_args),
        AgentHost::GenericMcp => configure_generic(options, store, mcp_server, server_args),
    }
}

fn configure_cli_host(
    options: HostSetupOptions,
    store: PathBuf,
    mcp_server: PathBuf,
    server_args: Vec<String>,
    claude: bool,
) -> Result<HostSetupReport> {
    let host_command = options.host.command_name().unwrap();
    let command_path = find_command(host_command);
    if command_path.is_none() && !options.dry_run {
        bail!(
            "{} CLI is not installed or not available on PATH; use --dry-run to print the configuration command",
            options.host.display_name()
        );
    }
    let command_path = command_path.unwrap_or_else(|| PathBuf::from(host_command));
    let existing = if options.dry_run {
        CliServerState::Absent
    } else {
        cli_server_state(&command_path, claude, &mcp_server, &server_args)?
    };

    let remove_args = vec![
        "mcp".to_string(),
        "remove".to_string(),
        SERVER_NAME.to_string(),
    ];
    let add_args = if claude {
        let mut args = vec![
            "mcp".to_string(),
            "add".to_string(),
            "--transport".to_string(),
            "stdio".to_string(),
            "--scope".to_string(),
            "local".to_string(),
            SERVER_NAME.to_string(),
            "--".to_string(),
            mcp_server.to_string_lossy().into_owned(),
        ];
        args.extend(server_args.clone());
        args
    } else {
        let mut args = vec![
            "mcp".to_string(),
            "add".to_string(),
            SERVER_NAME.to_string(),
            "--".to_string(),
            mcp_server.to_string_lossy().into_owned(),
        ];
        args.extend(server_args.clone());
        args
    };

    let mut changed = false;
    let action;
    if options.remove {
        action = "remove".to_string();
        if existing != CliServerState::Absent && !options.dry_run {
            run_checked(&command_path, &remove_args)?;
            changed = true;
        }
    } else if existing == CliServerState::Matching && !options.force {
        action = "already configured".to_string();
    } else if existing == CliServerState::Conflict && !options.force {
        bail!(
            "{} MCP server {SERVER_NAME} already exists with different settings; rerun with --force after reviewing the current configuration",
            options.host.display_name()
        );
    } else {
        action = if existing != CliServerState::Absent {
            "replace".to_string()
        } else {
            "add".to_string()
        };
        if !options.dry_run {
            if existing != CliServerState::Absent {
                run_checked(&command_path, &remove_args)?;
            }
            run_checked(&command_path, &add_args)?;
            if cli_server_state(&command_path, claude, &mcp_server, &server_args)?
                != CliServerState::Matching
            {
                let _ = run_checked(&command_path, &remove_args);
                bail!(
                    "{} did not retain the expected MCP server configuration; the new registration was rolled back",
                    options.host.display_name()
                );
            }
            changed = true;
        }
    }

    let command = render_command(
        &command_path,
        if options.remove {
            &remove_args
        } else {
            &add_args
        },
    );
    let rollback = render_command(&command_path, &remove_args);
    let mut notes = Vec::new();
    if claude {
        notes.push("Claude Code registration uses local project scope".to_string());
    } else {
        notes.push("Codex CLI and IDE share the registered MCP configuration".to_string());
    }

    Ok(HostSetupReport {
        host: options.host.display_name().to_string(),
        action,
        changed,
        dry_run: options.dry_run,
        server_name: SERVER_NAME.to_string(),
        store_path: store,
        mcp_server,
        config_path: None,
        command,
        rollback,
        smoke_ok: !options.remove && !options.dry_run,
        passphrase_env: options.passphrase_env,
        notes,
    })
}

fn configure_cursor(
    options: HostSetupOptions,
    store: PathBuf,
    mcp_server: PathBuf,
    server_args: Vec<String>,
) -> Result<HostSetupReport> {
    let config_path = env::current_dir()?.join(".cursor").join("mcp.json");
    let mut document = read_json_object(&config_path)?;
    let servers = object_entry(&mut document, "mcpServers")?;
    let desired = json!({
        "type": "stdio",
        "command": mcp_server.to_string_lossy(),
        "args": server_args
    });
    let existing = servers.get(SERVER_NAME).cloned();
    let mut changed = false;
    let action;

    if options.remove {
        action = "remove".to_string();
        if existing.is_some() && !options.dry_run {
            backup_if_exists(&config_path)?;
            servers.remove(SERVER_NAME);
            write_json_atomic(&config_path, &document)?;
            changed = true;
        }
    } else if existing.as_ref() == Some(&desired) {
        action = "already configured".to_string();
    } else if existing.is_some() && !options.force {
        bail!(
            "Cursor MCP server {SERVER_NAME} already exists with different settings; rerun with --force after reviewing the config"
        );
    } else {
        action = if existing.is_some() {
            "replace".to_string()
        } else {
            "add".to_string()
        };
        if !options.dry_run {
            backup_if_exists(&config_path)?;
            object_entry(&mut document, "mcpServers")?
                .insert(SERVER_NAME.to_string(), desired.clone());
            write_json_atomic(&config_path, &document)?;
            changed = true;
        }
    }

    let snippet = serde_json::to_string(&mcp_config_document(desired))?;
    Ok(HostSetupReport {
        host: options.host.display_name().to_string(),
        action,
        changed,
        dry_run: options.dry_run,
        server_name: SERVER_NAME.to_string(),
        store_path: store,
        mcp_server,
        config_path: Some(config_path.clone()),
        command: format!("merge into {}: {snippet}", config_path.display()),
        rollback: "mge setup cursor --remove (from the same project directory)".to_string(),
        smoke_ok: !options.remove && !options.dry_run,
        passphrase_env: options.passphrase_env,
        notes: vec![
            "Cursor project configuration is merged without replacing unrelated MCP servers"
                .to_string(),
            "An existing config is backed up before modification".to_string(),
        ],
    })
}

fn configure_generic(
    options: HostSetupOptions,
    store: PathBuf,
    mcp_server: PathBuf,
    server_args: Vec<String>,
) -> Result<HostSetupReport> {
    if options.remove {
        bail!("generic MCP mode only prints configuration and cannot remove host-owned settings");
    }
    let snippet = serde_json::to_string_pretty(&mcp_config_document(json!({
        "type": "stdio",
        "command": mcp_server.to_string_lossy(),
        "args": server_args
    })))?;
    Ok(HostSetupReport {
        host: options.host.display_name().to_string(),
        action: "print configuration".to_string(),
        changed: false,
        dry_run: true,
        server_name: SERVER_NAME.to_string(),
        store_path: store,
        mcp_server,
        config_path: None,
        command: snippet,
        rollback: "remove the memory-genome entry from the host MCP configuration".to_string(),
        smoke_ok: !options.dry_run,
        passphrase_env: options.passphrase_env,
        notes: vec!["No host configuration was modified".to_string()],
    })
}

fn server_args(store: &Path, passphrase_env: Option<&str>) -> Vec<String> {
    let mut args = vec!["--store".to_string(), store.to_string_lossy().into_owned()];
    if let Some(name) = passphrase_env {
        args.push("--passphrase-env".to_string());
        args.push(name.to_string());
    }
    args
}

fn mcp_config_document(server: Value) -> Value {
    let mut servers = Map::new();
    servers.insert(SERVER_NAME.to_string(), server);
    let mut root = Map::new();
    root.insert("mcpServers".to_string(), Value::Object(servers));
    Value::Object(root)
}

fn smoke_server(server: &Path, args: &[String]) -> Result<()> {
    let mut child = Command::new(server)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start MCP server {}", server.display()))?;
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mge-setup","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"mge_stats","arguments":{}}}),
    ];
    {
        let mut stdin = child.stdin.take().unwrap();
        for request in requests {
            serde_json::to_writer(&mut stdin, &request)?;
            stdin.write_all(b"\n")?;
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "MCP smoke failed for {}: {}",
            server.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let responses = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if responses.len() != 3
        || responses[0]["result"]["serverInfo"]["name"] != "memory-genome-engine"
        || !responses[1]["result"]["tools"].is_array()
        || responses[2]["result"]["isError"] != false
    {
        bail!("MCP smoke returned an invalid initialize/tools/stats sequence");
    }
    Ok(())
}

fn resolve_mcp_server(explicit: Option<&Path>) -> Result<PathBuf> {
    let candidate = if let Some(path) = explicit {
        path.to_path_buf()
    } else if let Some(path) = env::var_os("MGE_MCP_SERVER_BIN") {
        PathBuf::from(path)
    } else {
        let current = env::current_exe()?;
        let sibling = current.with_file_name(executable_name("mge-mcp-server"));
        if sibling.is_file() {
            sibling
        } else {
            find_command("mge-mcp-server").ok_or_else(|| {
                anyhow!(
                    "mge-mcp-server was not found next to mge or on PATH; pass --mcp-server <PATH>"
                )
            })?
        }
    };
    if !candidate.is_file() {
        bail!("MCP server binary does not exist: {}", candidate.display());
    }
    absolute_path(&candidate)
}

fn resolve_mcp_server_for_removal(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return absolute_path(path);
    }
    let current = env::current_exe()?;
    let sibling = current.with_file_name(executable_name("mge-mcp-server"));
    if sibling.is_file() {
        return absolute_path(&sibling);
    }
    Ok(find_command("mge-mcp-server")
        .unwrap_or_else(|| PathBuf::from(executable_name("mge-mcp-server"))))
}

fn executable_name(name: &str) -> OsString {
    if cfg!(windows) {
        format!("{name}.exe").into()
    } else {
        name.into()
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return path
            .canonicalize()
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    Ok(absolute)
}

fn find_command(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let names = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_string(),
        ]
    } else {
        vec![name.to_string()]
    };
    env::split_paths(&path)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

fn cli_server_state(
    command: &Path,
    claude: bool,
    mcp_server: &Path,
    server_args: &[String],
) -> Result<CliServerState> {
    let mut process = Command::new(command);
    process.args(["mcp", "get", SERVER_NAME]);
    if !claude {
        process.arg("--json");
    }
    let output = process
        .output()
        .with_context(|| format!("failed to inspect {} MCP configuration", command.display()))?;
    if !output.status.success() {
        return Ok(CliServerState::Absent);
    }
    if claude {
        let current = normalized_text(&String::from_utf8_lossy(&output.stdout));
        let expected_command = normalized_text(&mcp_server.to_string_lossy());
        let matches = current.contains(&expected_command)
            && server_args
                .iter()
                .all(|argument| current.contains(&normalized_text(argument)));
        return Ok(if matches {
            CliServerState::Matching
        } else {
            CliServerState::Conflict
        });
    }

    let current: Value = serde_json::from_slice(&output.stdout)
        .context("Codex returned invalid JSON for the current MCP configuration")?;
    let configured_command = current["transport"]["command"].as_str().unwrap_or_default();
    let configured_args = current["transport"]["args"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let matches = normalized_text(configured_command)
        == normalized_text(&mcp_server.to_string_lossy())
        && configured_args.len() == server_args.len()
        && configured_args
            .iter()
            .zip(server_args)
            .all(|(left, right)| normalized_text(left) == normalized_text(right));
    Ok(if matches {
        CliServerState::Matching
    } else {
        CliServerState::Conflict
    })
}

fn normalized_text(value: &str) -> String {
    value.replace('\\', "/").to_lowercase()
}

fn run_checked(command: &Path, args: &[String]) -> Result<()> {
    let output = Command::new(command)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {}", command.display()))?;
    if !output.status.success() {
        bail!(
            "command failed: {}\n{}",
            render_command(command, args),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn render_command(command: &Path, args: &[String]) -> String {
    std::iter::once(command.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .map(|part| shell_quote(&part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-._/:\\".contains(ch))
    {
        value.to_string()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn read_json_object(path: &Path) -> Result<Map<String, Value>> {
    if !path.exists() {
        return Ok(Map::new());
    }
    let value: Value = serde_json::from_slice(&fs::read(path)?)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("{} must contain a JSON object", path.display()))
}

fn object_entry<'a>(
    document: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>> {
    let value = document
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    value
        .as_object_mut()
        .ok_or_else(|| anyhow!("{key} must be a JSON object"))
}

fn backup_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup = path.with_extension(format!("json.mge-backup-{timestamp}"));
    fs::copy(path, &backup).with_context(|| {
        format!(
            "failed to back up {} to {}",
            path.display(),
            backup.display()
        )
    })?;
    Ok(Some(backup))
}

fn write_json_atomic(path: &Path, document: &Map<String, Value>) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.mge-tmp");
    let bytes = serde_json::to_vec_pretty(&Value::Object(document.clone()))?;
    fs::write(&temporary, bytes)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&temporary, path)?;
    Ok(())
}

impl FromStr for AgentHost {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "codex" => Ok(Self::Codex),
            "claude-code" | "claude" => Ok(Self::ClaudeCode),
            "cursor" => Ok(Self::Cursor),
            "generic-mcp" | "generic" => Ok(Self::GenericMcp),
            other => bail!(
                "unknown agent host {other}; supported: codex, claude-code, cursor, generic-mcp"
            ),
        }
    }
}
