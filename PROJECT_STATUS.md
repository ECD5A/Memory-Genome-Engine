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

- Pre-Mandate 4 Documentation Tree Cleanup is complete in this cleanup pass.
- Next recommended mandate: Mandate 4 Product UI / Packaging.
- Optional pre-Mandate-4 check: real user corpus run plus real host compatibility check.

## Documentation Map

- [README](README.md): short product entrypoint and navigation.
- [Quickstart](QUICKSTART.md): first commands and core CLI workflow.
- [Architecture](docs/ARCHITECTURE.md): core design and storage/index model.
- [Security](docs/SECURITY.md): encryption, threat model, plaintext metadata risks.
- [Integration / MCP / SDK](docs/INTEGRATION.md): agent lifecycle, JSON-RPC adapter contract, and Python/TypeScript thin wrappers.
- [Benchmarks](docs/BENCHMARKS.md): performance tools and report interpretation.
- [Release](docs/RELEASE.md): build, smoke, and packaging checks.

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

## Current Known Limitations

- No product UI yet.
- No vector database.
- No encrypted indexes or blind marker metadata yet.
- No encrypted Markdown export yet.
- No package publishing yet.
- No external MCP SDK dependency.
- No automatic migration from unencrypted stores to encrypted stores.
- Larger user-provided corpus testing is still useful before any new performance work.

## Latest Verification Baseline

Last full verification before this docs cleanup:

- `cargo fmt`: passed.
- `cargo test`: passed, 135 tests.
- CLI unencrypted smoke: passed.
- CLI encrypted hot+sealed smoke: passed.
- Encrypted reopen recall smoke: passed.
- Encrypted validate/rebuild smoke: passed.
- MCP encrypted smoke: passed.
- Python SDK encrypted smoke: passed.
- TypeScript SDK encrypted smoke: passed.
- Rust example smoke: passed.

This documentation cleanup changes Markdown only. Core/storage/codec/filter/recall/security behavior is unchanged.

## Next Recommended Step

Start Mandate 4: Product UI / Packaging.

Optional pre-Mandate-4 work:

- Run a larger real user corpus through `mge-corpus-bench`.
- Run the JSON-RPC adapter against a real local host/agent runner.
- Decide release packaging target and distribution format.
