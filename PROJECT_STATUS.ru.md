# Статус Проекта

[English version](PROJECT_STATUS.md)

Этот файл - статусный source of truth. Usage, architecture, security, integration, SDK, benchmark и release instructions вынесены в отдельные docs по ссылкам ниже.

## Текущее Состояние

Memory Genome Engine - Rust-first локальный memory engine для агентов.

Закрыто:

- Mandate 1: Developer-ready Core.
- Mandate 2: Agent Integration / MCP / SDK.
- Mandate 3: Security / Encryption.

Текущий этап:

- Mandate 4: Product UI / Packaging в работе.
- Текущий фокус: terminal product UI, packaging/dev UX, release scripts и read-only local diagnostics.

## Карта Документации

- [README](README.ru.md): короткая продуктовая точка входа и навигация.
- [Quickstart](QUICKSTART.ru.md): первые команды и основной CLI workflow.
- [Architecture](docs/ARCHITECTURE.ru.md): core design, storage и index model.
- [Security](docs/SECURITY.ru.md): encryption, threat model, plaintext metadata risks.
- [Integration / MCP / SDK](docs/INTEGRATION.ru.md): agent lifecycle, JSON-RPC adapter contract и Python/TypeScript thin wrappers.
- [Release / Benchmarks](docs/RELEASE.ru.md): build, smoke, packaging checks, performance tools и чтение отчётов.

## Roadmap Snapshot

| Stage | Status | Notes |
| --- | --- | --- |
| v0.1 core/CLI | Closed | Rust core, CLI, `MemoryCell`, `MarkerDictionary`, hot memory, sealed pages, exact index, recall и `ContextPacket` работают. |
| v0.2 storage/index foundation | Closed | Binary runtime storage, MessagePack pages, zstd, binary headers/checksums, L1 Hot RAM, validation/rebuild, Binary Fuse opt-in и benchmark foundation готовы. |
| v0.3 SDK/MCP | Closed | Local JSON-RPC MCP-ready adapter, thin Python SDK, thin TypeScript SDK, typed contracts, examples и smokes готовы. |
| v0.4 security | Closed / future hardening | Encrypted store mode, session unlock, encrypted hot log, encrypted snapshot, encrypted sealed page payloads и metadata risk memo готовы. |
| v0.5 safety/search | Partial foundation | Policy/capabilities и audit hook foundation есть; poisoning/conflict detection и optional vector reranking - future work. |

## Mandate 1 Closure Status

Developer-ready core закрыт.

Готово:

- Binary runtime storage layout: `.mgm`, `.mgd`, `.mgl`, `.mgs`, `.mgp`, `.mgi`.
- L1 Hot RAM с exact mutable indexes и durable hot-log recovery.
- Явный `MarkerGenome`; `MemoryCell.markers` остаётся flattened runtime/index compatibility view.
- Sealed pages, page catalog metadata, metadata pruning, decoded page cache и runtime scoring cache.
- `ExactMarkerPageIndex` default; `BinaryFusePageIndex` optional и benchmark-gated.
- Focused, broad и full-scope recall.
- Deep validation и safe catalog/index rebuild.
- Synthetic и corpus benchmark harnesses.

Core constraints:

- JSON/JSONL не являются runtime storage.
- Custom page codec, vector reranking и новые filter families отложены до benchmark-доказательства.
- `CandidatePageIndex`, recall modes и storage layout нельзя менять без отдельного решения.

## Mandate 2 Closure Status

Local agent integration закрыт.

Готово:

- `mge-mcp-server`: versioned local JSON-RPC stdin/stdout adapter.
- `protocol_version = mge-jsonrpc-1`.
- `integration_schema_version = 1`.
- Tool contracts для remember, recall, seal, checkpoint, stats, validate, rebuild indexes и Markdown export.
- Structured error model со стабильным `error.details.error_kind`.
- Python SDK thin wrapper over Rust CLI.
- TypeScript SDK thin wrapper over Rust CLI.
- Agent host examples для CLI/Rust, Python, TypeScript и MCP-style JSON-RPC.

Намеренно не реализовано:

- External MCP SDK dependency.
- Package publishing.
- PyO3/maturin native Python binding.
- Remote service hosting.

## Mandate 3 Closure Status

Security/encryption-ready layer закрыт.

Готово:

- Opt-in encrypted store mode через `mge init --encrypted`.
- Session unlock через `--passphrase-env`.
- KDF/AEAD crates: `argon2`, `chacha20poly1305`, `rand`, `zeroize`.
- Encrypted/authenticated `hot/hot.mgl` hot record payloads.
- Encrypted/authenticated `hot/snapshot.mgs` checkpoint payloads.
- Encrypted/authenticated `pages/*.mgp` sealed page payloads.
- MCP/Python/TypeScript passphrase environment variable passthrough.
- `validate --deep` и `rebuild-indexes` работают после unlock с encrypted sealed pages.
- Locked encrypted stores возвращают `store_locked`; wrong key или AEAD failure возвращают `auth_failed` / authentication failure.
- Нет silent plaintext fallback для encrypted-mode payload operations.
- Plaintext metadata risk analysis задокументирован.

Plaintext by design:

- `manifest.mgm` safe metadata и key-derivation parameters.
- Binary frame headers.
- `dictionary/markers.mgd`.
- `indexes/*.mgi` и page catalog summaries.
- Encoded page sizes и marker/scope/kind/status/sensitivity/trust summaries.
- Markdown export, если пользователь явно его создаёт.
- Process memory и `ContextPacket` во время unlocked session.

Future security work не блокирует текущий продукт:

- Optional keyed marker fingerprint prototype.
- Optional blind marker metadata/index design после benchmark и migration evidence.
- Optional encrypted Markdown export.
- Optional interactive unlock / host key-management integration.
- Explicit migration tool from unencrypted stores to encrypted stores.

## Mandate 4 Status

Product UI / Packaging в работе.

Текущий package:

- Human-first terminal interface через `mge tui` на `ratatui` + `crossterm`.
- TUI screens: dashboard, recall, add memory, seal/checkpoint, status/diagnostics, index benchmark, Markdown export/import status, settings и help.
- Runtime EN/RU language switching через F1, L/l и Д/д.
- Thin `mge-cli` app service layer для TUI и CLI-oriented diagnostics.
- Read-only `mge doctor` diagnostics для store structure, manifest/security state, required files, optional unlock и explicit deep validation.
- Repo-local release build scripts:
  - `scripts/build-release.sh`
  - `scripts/build-release.ps1`
- Repo-local release smoke scripts:
  - `scripts/smoke-release.sh`
  - `scripts/smoke-release.ps1`
- Local encrypted demo workflow scripts:
  - `scripts/demo-local-memory.sh`
  - `scripts/demo-local-memory.ps1`

Намеренно не реализовано:

- Package publishing.
- External MCP SDK dependency.
- Heavy UI framework.
- Web/desktop GUI.
- Storage, codec, filter, recall или encryption format changes.

## Текущие Ограничения

- Human UI сейчас terminal-first через `mge tui`; web/desktop GUI нет.
- Vector database нет.
- Encrypted indexes и blind marker metadata пока нет.
- Encrypted Markdown export пока нет.
- Package publishing пока нет.
- External MCP SDK dependency не добавлена.
- Automatic migration from unencrypted stores to encrypted stores пока нет.
- Большой user-provided corpus всё ещё полезен перед новой performance work.

## Последняя Verification Baseline

Последняя Mandate 4 TUI/package проверка:

- `cargo fmt --check`: passed.
- `cargo test`: passed, 147 tests.
- `cargo build -p mge-cli --bins`: passed.
- CLI quickstart smoke: passed на временном store.
- TUI help smoke: `cargo run -p mge-cli -- tui --help` passed.

Предыдущая packaging/dev UX baseline из release-script pass остаётся актуальной:

- `cargo build -p mge-cli --bins --release`: passed.
- `scripts/build-release.ps1`: passed.
- `scripts/smoke-release.ps1`: passed.
- `scripts/demo-local-memory.ps1`: passed.

Mandate 4 добавляет terminal UI, packaging/dev UX и read-only diagnostics. Storage/codec/filter/recall/security formats не менялись.

## Следующий Рекомендуемый Шаг

Завершить первый Mandate 4 TUI package, затем перейти к packaging target selection и product distribution design.
