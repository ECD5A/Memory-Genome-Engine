# Project Status

[Russian version](PROJECT_STATUS.ru.md)

This file is the status source of truth. Usage, architecture, security, integration, SDK, benchmark, and release instructions live in the dedicated docs linked below.

## Current Summary

Memory Genome Engine is a Rust-first local memory engine for agents.

Closed:

- Mandate 1: Developer-ready Core.
- Mandate 2: Agent Integration / MCP / SDK.
- Mandate 3: Security / Encryption.
- Mandate 4: Product UI / Packaging.

Current stage:

- Mandate 5: Product Distribution / Installers / Release Targets is active.
- Goal: prepare clean local release/install artifacts and GitHub repository polish without new core features.

## Documentation Map

- [README](README.md): short product entrypoint and navigation.
- [Quickstart](QUICKSTART.md): first commands and core CLI workflow.
- [Architecture](docs/ARCHITECTURE.md): core design and storage/index model.
- [Security](docs/SECURITY.md): encryption, threat model, plaintext metadata risks.
- [Integration / MCP / SDK](docs/INTEGRATION.md): agent lifecycle, JSON-RPC adapter contract, and Python/TypeScript thin wrappers.
- [Release / Benchmarks](docs/RELEASE.md): build, smoke, packaging checks, performance tools, and report interpretation.

## Roadmap Snapshot

| Stage | Status | Notes |
| --- | --- | --- |
| v0.1 core/CLI | Closed | Rust core, CLI, `MemoryCell`, `MarkerDictionary`, hot memory, sealed pages, exact index, recall, and `ContextPacket` work. |
| v0.2 storage/index foundation | Closed | Binary runtime storage, MessagePack pages, zstd, binary headers/checksums, L1 Hot RAM, validation/rebuild, Binary Fuse opt-in, and benchmark foundation are ready. |
| v0.3 SDK/MCP | Closed | Local JSON-RPC MCP-ready adapter, thin Python SDK, thin TypeScript SDK, typed contracts, examples, and smokes are ready. |
| v0.4 security | Closed / future hardening | Encrypted store mode, session unlock, encrypted hot log, encrypted snapshot, encrypted sealed page payloads, and metadata risk memo are ready. |
| v0.5 safety/search | Partial foundation | Policy/capabilities and audit hook foundation exist; poisoning/conflict detection and optional vector reranking are future work. |

## Mandate 1 Closure Status

Developer-ready core is closed.

Ready:

- Binary runtime storage layout: `.mgm`, `.mgd`, `.mgl`, `.mgs`, `.mgp`, `.mgi`.
- L1 Hot RAM with exact mutable indexes and durable hot-log recovery.
- Explicit `MarkerGenome`; `MemoryCell.markers` remains as flattened runtime/index compatibility view.
- Sealed pages, page catalog metadata, metadata pruning, decoded page cache, and runtime scoring cache.
- `ExactMarkerPageIndex` default; `BinaryFusePageIndex` optional and benchmark-gated.
- Focused, broad, and full-scope recall.
- Deep validation and safe catalog/index rebuild.
- Synthetic and corpus benchmark harnesses.

Core constraints:

- JSON/JSONL are not runtime storage.
- Custom page codec, vector reranking, and new filter families are deferred until benchmark evidence exists.
- `CandidatePageIndex`, recall modes, and storage layout should not be changed casually.

## Mandate 2 Closure Status

Local agent integration is closed.

Ready:

- `mge-mcp-server`: versioned local JSON-RPC stdin/stdout adapter.
- `protocol_version = mge-jsonrpc-1`.
- `integration_schema_version = 1`.
- Tool contracts for remember, recall, seal, checkpoint, stats, validate, rebuild indexes, and Markdown export.
- Structured error model with stable `error.details.error_kind`.
- Python SDK thin wrapper over Rust CLI.
- TypeScript SDK thin wrapper over Rust CLI.
- Agent host examples for CLI/Rust, Python, TypeScript, and MCP-style JSON-RPC.

Intentionally not implemented:

- External MCP SDK dependency.
- Package publishing.
- PyO3/maturin native Python binding.
- Remote service hosting.

## Mandate 3 Closure Status

Security/encryption-ready layer is closed.

Ready:

- Opt-in encrypted store mode through `mge init --encrypted`.
- Session unlock through `--passphrase-env`.
- KDF/AEAD crates: `argon2`, `chacha20poly1305`, `rand`, `zeroize`.
- Encrypted/authenticated `hot/hot.mgl` hot record payloads.
- Encrypted/authenticated `hot/snapshot.mgs` checkpoint payloads.
- Encrypted/authenticated `pages/*.mgp` sealed page payloads.
- MCP/Python/TypeScript passphrase environment variable passthrough.
- `validate --deep` and `rebuild-indexes` work after unlock with encrypted sealed pages.
- Locked encrypted stores return `store_locked`; wrong keys or AEAD failures return `auth_failed` / authentication failure.
- No silent plaintext fallback for encrypted-mode payload operations.
- Plaintext metadata risk analysis is documented.

Plaintext by design:

- `manifest.mgm` safe metadata and key-derivation parameters.
- Binary frame headers.
- `dictionary/markers.mgd`.
- `indexes/*.mgi` and page catalog summaries.
- Encoded page sizes and marker/scope/kind/status/sensitivity/trust summaries.
- Markdown export if explicitly created.
- Process memory and `ContextPacket` while unlocked.

Future security work is non-blocking:

- Optional keyed marker fingerprint prototype.
- Optional blind marker metadata/index design after benchmark and migration evidence.
- Optional encrypted Markdown export.
- Optional interactive unlock / host key-management integration.
- Explicit migration tool from unencrypted stores to encrypted stores.

## Mandate 4 Closure Status

Product UI / Packaging is closed.

Ready:

- Human-first terminal interface through `mge tui` using `ratatui` + `crossterm`.
- First-run setup helper through `mge setup`.
- TUI setup wizard for first launch.
- TUI screens for dashboard, recall, add memory, seal/checkpoint, status/diagnostics/doctor, index benchmark, Markdown export/import status, settings, and help.
- Runtime EN/RU language switching with F1, L/l, and D/d.
- Safe encrypted setup guidance through `--passphrase-env`; passphrases are read from the environment, not typed into the TUI.
- Thin `mge-cli` app service layer shared by the TUI and CLI-oriented diagnostics.
- Read-only `mge doctor` diagnostics for store structure, manifest/security state, required files, optional unlock, and explicit deep validation.
- Repo-local release build scripts:
  - `scripts/build-release.sh`
  - `scripts/build-release.ps1`
- Repo-local release smoke scripts:
  - `scripts/smoke-release.sh`
  - `scripts/smoke-release.ps1`
- Release scripts build `cargo build -p mge-cli --bins --release`, verify release binaries, and run CLI/encrypted/MCP/SDK smoke checks without publishing packages or committing artifacts.
- Windows PowerShell build/smoke scripts are verified on this host.
- Linux `.sh` build/smoke scripts are verified through WSL Ubuntu in Mandate 5; macOS has not been locally executed on this Windows host.
- CLI, MCP JSON-RPC, Python SDK, TypeScript SDK, and Rust example smokes pass through the release smoke script.
- Local encrypted demo workflow scripts:
  - `scripts/demo-local-memory.sh`
  - `scripts/demo-local-memory.ps1`

Still intentionally not implemented:

- Package publishing.
- External MCP SDK dependency.
- Heavy UI framework.
- Storage, codec, filter, recall, or encryption format changes.
- Markdown import remains disabled; Markdown export is supported and plaintext by design.
- Full interactive TUI real-TTY end-to-end automation is not implemented; TUI behavior is covered by unit tests, help smokes, and manual terminal checks.

## Current Known Limitations

- No web/desktop GUI; Mandate 4 UI is terminal-first through `mge tui`.
- No full interactive real-TTY TUI e2e automation yet.
- No vector database.
- No encrypted indexes or blind marker metadata yet.
- No encrypted Markdown export yet.
- No package publishing yet.
- Linux `.sh` release scripts are verified through WSL Ubuntu; macOS `.sh` execution is still not locally verified on this Windows machine.
- Markdown import is disabled.
- No external MCP SDK dependency.
- No automatic migration from unencrypted stores to encrypted stores.
- Larger user-provided corpus testing is still useful before any new performance work.

## Latest Verification Baseline

Latest Mandate 5 distribution verification:

- `cargo fmt --check`: passed.
- `cargo test`: passed, 157 tests.
- `cargo check -p mge-cli --bins`: passed.
- `cargo build -p mge-cli --bins --release`: passed.
- `scripts/build-release.ps1`: passed and created `target/mge-release/windows-x64`.
- `scripts/smoke-release.ps1`: passed.
- `scripts/install.ps1 -NoBuild`: passed on a temporary install directory.
- Release binary TUI help smoke: `mge tui --help` passed.
- Release binary setup help smoke: `mge setup --help` passed.
- CLI quickstart smoke: passed on a temporary store through release script.
- Encrypted smoke: passed through `MGE_RELEASE_SMOKE_PASSPHRASE`.
- MCP JSON-RPC smoke: passed for `mge_schema` and `mge_stats`.
- Python SDK smoke: passed when run from `scripts/smoke-release.ps1`.
- TypeScript SDK smoke: passed when run from `scripts/smoke-release.ps1`.
- Rust agent host example smoke: passed when run from `scripts/smoke-release.ps1`.
- Markdown link sanity: passed.
- `git diff --check`: passed.
- WSL Ubuntu environment: `rustc 1.96.0`, `cargo 1.96.0`, GNU bash 5.3.9, GCC 15.2.0, `pkg-config` 2.5.1.
- `scripts/build-release.sh` through WSL Ubuntu: passed with `CARGO_TARGET_DIR=target/wsl-release`.
- `scripts/smoke-release.sh` through WSL Ubuntu: passed with `CARGO_TARGET_DIR=target/wsl-release`.
- `scripts/install.sh --help` through WSL Ubuntu: passed.

Mandate 5 has local Windows and WSL Ubuntu release/build/smoke/install coverage. macOS shell execution still needs a macOS host. Storage/codec/filter/recall/security formats remain unchanged.

## Mandate 5 Distribution Status

Mandate 5 is active.

Added in this distribution pass:

- Local release layout generation under `target/mge-release/<platform>/`.
- User-local install scripts:
  - `scripts/install.sh`
  - `scripts/install.ps1`
- GitHub community files:
  - `SECURITY.md`
  - `CONTRIBUTING.md`
  - `CODE_OF_CONDUCT.md`
- Release docs for build, smoke, install, and local layout behavior.

Mandate 5 rules:

- No package publishing yet.
- No committed binaries or generated stores.
- No core/storage/codec/filter/recall/security behavior changes.
- Install scripts copy locally built binaries only and do not require admin/root privileges.

Current platform note:

- Windows PowerShell release scripts are locally verified on this host.
- WSL Ubuntu release scripts are locally verified after installing the minimal Linux build toolchain and Rust stable.
- Python and Node were not installed in WSL; optional Python/TypeScript SDK smokes were skipped there. They remain covered by the Windows release smoke.
- macOS shell scripts are expected to follow the POSIX path but are not locally executed on this Windows machine.

## Next Recommended Step

Finish Mandate 5 release verification, then prepare a GitHub release candidate.
