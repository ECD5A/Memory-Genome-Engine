# Quickstart

This guide shows the shortest path to a working local Memory Genome store.

## Use A Release Binary

Download and verify the latest release into a user-local bin directory:

```powershell
Invoke-WebRequest https://raw.githubusercontent.com/ECD5A/Memory-Genome-Engine/main/scripts/install-release.ps1 -OutFile install-release.ps1
powershell -ExecutionPolicy Bypass -File .\install-release.ps1
```

```bash
curl --fail --location --remote-name https://raw.githubusercontent.com/ECD5A/Memory-Genome-Engine/main/scripts/install-release.sh
bash install-release.sh
```

The installers verify the selected archive against the release `SHA256SUMS` before extracting `mge` and `mge-mcp-server`. They do not require admin/root privileges or modify shell profiles. A fixed release can be selected with `-Version v0.1.1` or `--version v0.1.1`.

Archives can also be downloaded manually from:

```text
https://github.com/ECD5A/Memory-Genome-Engine/releases
```

After installation, run:

```bash
mge --help
mge setup
mge tui
```

On Windows, the binary is `mge.exe`. On Linux and macOS, it is `mge`. Windows and WSL/Linux are verified locally; macOS remains enabled in CI and release automation but is not locally executed on this Windows development host.

Most commands below use `cargo run -p mge-cli --` because they also work from a clean source checkout. If you use a release binary, replace:

```bash
cargo run -p mge-cli --
```

with:

```bash
mge
```

## Build From Source

```bash
git clone https://github.com/ECD5A/Memory-Genome-Engine.git
cd Memory-Genome-Engine
cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server
```

The main CLI binary is `mge`:

```bash
cargo run -p mge-cli -- --help
```

Human terminal interface:

```bash
cargo run -p mge-cli -- tui
```

Inside the TUI, use arrows, Enter, Space, Esc, F1/L/D/Д for language, and F2 for help. Scriptable CLI commands remain unchanged.

First-run setup helper:

```bash
cargo run -p mge-cli -- setup
cargo run -p mge-cli -- setup --encrypted --passphrase-env MGE_PASSPHRASE
```

Encrypted setup reads the passphrase from the environment variable name passed to `--passphrase-env`; the passphrase itself is not typed into the TUI or printed.
Encryption is a store-creation choice. Re-running `setup --encrypted` or `init --encrypted` never silently converts an existing plaintext store.

Register the initialized store with a local agent host:

```bash
mge setup codex
mge setup claude-code
mge setup cursor
mge setup generic-mcp
```

The first three commands update only the selected host registration. Generic mode prints a portable MCP stdio configuration. Add `--dry-run` to inspect a native command first or `--remove` to undo the registration. See [Integration](docs/INTEGRATION.md) for encrypted-host setup and rollback details.

## Create A Store

Default store:

```bash
cargo run -p mge-cli -- init
```

Fast profile with compact sealed pages:

```bash
cargo run -p mge-cli -- init --profile fast
```

The runtime store is binary. JSON is not used as runtime storage.

## Remember

```bash
cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed
```

Structured value:

```bash
cargo run -p mge-cli -- remember \
  --kind user_preference \
  --subject answer_style \
  --json-value '{"style":"concise","max_examples":2}'
```

Reference value for sensitive placeholders:

```bash
cargo run -p mge-cli -- remember \
  --kind project_fact \
  --reference-value vault://references/api-key \
  --sensitivity secret_reference
```

Provenance and links:

```bash
cargo run -p mge-cli -- remember "Decision recorded" \
  --kind decision \
  --source-type issue \
  --source-ref MGE-1 \
  --link 1
```

Deterministic agent-session ingestion keeps turn boundaries and creates bounded memory cells:

```bash
cargo run -p mge-cli -- remember-session \
  --session-id task-42 \
  --scope my_project \
  --turn "user=Review the release plan" \
  --turn "assistant=Use a staged rollout" \
  --turn "user=Keep the rollback requirement"
```

One-time Markdown import for migrating existing notes:

```bash
cargo run -p mge-cli -- import markdown ./notes --scope my_project --marker source:notes
```

Markdown import writes normal binary `MemoryCell` records. Markdown is not runtime storage.

## Recall

```bash
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
```

Recall modes:

```bash
cargo run -p mge-cli -- recall "technical answer style" --mode focused
cargo run -p mge-cli -- recall "project context" --mode broad
cargo run -p mge-cli -- recall --mode full-scope --scope global
```

Use explicit markers for deterministic recall:

```bash
cargo run -p mge-cli -- recall "answer style" --marker kind:user_preference --marker scope:global
```

## Seal, Validate, Export

```bash
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- doctor --store .memory-genome
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- rebuild-indexes
cargo run -p mge-cli -- export
```

Markdown export is human-readable and plaintext by design.

Soft memory maintenance without rewriting sealed pages:

```bash
cargo run -p mge-cli -- mark 1 --status rejected
cargo run -p mge-cli -- mark 1 --status active
```

`rejected`, `deprecated`, and `superseded` overrides hide memory from normal recall. `active` clears the override. Sealed page payloads are not rewritten.

`mge doctor` is read-only by default. Use `--deep` only when you explicitly want validation work:

```bash
cargo run -p mge-cli -- doctor --store .memory-genome --deep
```

## Encrypted Store

Encrypted mode is opt-in:

```bash
export MGE_PASSPHRASE="use-a-real-secret"
cargo run -p mge-cli -- init --encrypted --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- remember "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- checkpoint --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- seal --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- recall "private memory" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- validate --deep --passphrase-env MGE_PASSPHRASE
```

Encrypted mode protects hot log payloads, checkpoint payloads, and sealed page payloads. Marker dictionary, indexes, catalog summaries, Markdown export, and process memory remain plaintext by design. See [Security](docs/SECURITY.md).

Local encrypted demo workflow:

```bash
./scripts/demo-local-memory.sh
# or on Windows:
powershell -ExecutionPolicy Bypass -File scripts/demo-local-memory.ps1
```

The demo runs two independent CLI sessions against one encrypted store: the first records and seals a decision, and the second reopens and recalls it. It calls no external APIs and removes the temporary store by default. Set `KEEP_MGE_DEMO=1` to retain the store for inspection.

## Storage Defaults

Compact page storage for a new store:

```bash
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

Change defaults for future sealed pages in an existing store:

```bash
cargo run -p mge-cli -- config show
cargo run -p mge-cli -- config set --page-codec messagepack --compression zstd
cargo run -p mge-cli -- config set --page-clusterer marker_overlap
```

Existing sealed pages are not rewritten when config defaults change.

## Optional Binary Fuse Index

Exact marker page index is the default. Binary Fuse is opt-in:

```bash
cargo run -p mge-cli -- init --index-kind binary_fuse_page
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
```

Switching index kind rebuilds candidate indexes from existing sealed pages; it does not rewrite page payloads.

## Local Integration

Initialize the store with the CLI/setup flow before connecting an agent host:

```bash
cargo run -p mge-cli -- init --profile fast
```

Run the JSON-RPC adapter:

```bash
cargo run -p mge-cli --bin mge-mcp-server
```

The MCP server implements the standard stdio lifecycle (`initialize`, `tools/list`, `tools/call`) and keeps direct `mge_*` JSON-RPC methods for compatibility. It operates on an existing `store_path`; bootstrap the store once through CLI/setup, then use MCP tools for session ingestion, recall, checkpoint, seal, validation, index rebuild, and export.

Run SDK examples:

```bash
python examples/python_basic_usage.py
node examples/typescript_basic_usage.ts
```

More integration details:

- [Integration / MCP / SDK](docs/INTEGRATION.md)

## Release Smoke

For local release readiness without publishing packages or committing binaries:

```bash
./scripts/build-release.sh
./scripts/smoke-release.sh
./scripts/install.sh --install-dir "$HOME/.local/bin"
# or on Windows:
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
powershell -ExecutionPolicy Bypass -File scripts/smoke-release.ps1
powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -InstallDir "$env:USERPROFILE\.local\bin"
```

Details: [Release](docs/RELEASE.md).
