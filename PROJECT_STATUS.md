# Project Status

[Russian version](PROJECT_STATUS.ru.md)

This file is the status source of truth. Usage, architecture, security, integration, SDK, benchmark, and release instructions live in the dedicated docs linked below.

## Current Summary

Memory Genome Engine is a Rust-first local memory engine for agents.

Closed:

- Mandate 1: Developer-ready Core.
- Mandate 2: Agent Integration / MCP / SDK.
- Mandate 3: Security / Encryption.

Current stage:

- Mandate 4: Product UI / Packaging is in progress.
- Current focus: packaging/dev UX, release scripts, and read-only local diagnostics.

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

## Mandate 4 Status

Product UI / Packaging is in progress.

Current package:

- Read-only `mge doctor` diagnostics for store structure, manifest/security state, required files, optional unlock, and explicit deep validation.
- Repo-local release build scripts:
  - `scripts/build-release.sh`
  - `scripts/build-release.ps1`
- Repo-local release smoke scripts:
  - `scripts/smoke-release.sh`
  - `scripts/smoke-release.ps1`
- Local encrypted demo workflow scripts:
  - `scripts/demo-local-memory.sh`
  - `scripts/demo-local-memory.ps1`

Still intentionally not implemented:

- Package publishing.
- External MCP SDK dependency.
- Heavy UI framework.
- Storage, codec, filter, recall, or encryption format changes.

## Current Known Limitations

- Product UI is not started yet; Mandate 4 is currently packaging/dev UX.
- No vector database.
- No encrypted indexes or blind marker metadata yet.
- No encrypted Markdown export yet.
- No package publishing yet.
- No external MCP SDK dependency.
- No automatic migration from unencrypted stores to encrypted stores.
- Larger user-provided corpus testing is still useful before any new performance work.

## Latest Verification Baseline

Latest Mandate 4 packaging/dev UX verification:

- `cargo fmt`: passed.
- `cargo test`: passed, 137 tests.
- `cargo build -p mge-cli --bins`: passed.
- `cargo build -p mge-cli --bins --release`: passed.
- `scripts/build-release.ps1`: passed.
- `scripts/smoke-release.ps1`: passed.
- `scripts/demo-local-memory.ps1`: passed.
- CLI quickstart smoke: passed through release smoke.
- Encrypted quickstart smoke: passed through release smoke and demo.
- MCP smoke: passed through release smoke.
- Python SDK smoke: passed through release smoke.
- TypeScript SDK smoke: passed through release smoke.
- Rust CLI host example smoke: passed through release smoke.

Mandate 4 adds packaging/dev UX and read-only diagnostics. Storage/codec/filter/recall/security formats remain unchanged.

## Next Recommended Step

Continue Mandate 4 with packaging target selection and product distribution design.
