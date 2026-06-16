# Статус Проекта

[English version](PROJECT_STATUS.md)

Этот файл - статусный source of truth. Usage, architecture, security, integration, SDK, benchmark и release instructions вынесены в отдельные docs по ссылкам ниже.

## Текущее Состояние

Memory Genome Engine - Rust-first локальный memory engine для агентов.

Закрыто:

- Mandate 1: Developer-ready Core.
- Mandate 2: Agent Integration / MCP / SDK.
- Mandate 3: Security / Encryption.
- Mandate 4: Product UI / Packaging.

Текущий этап:

- Mandate 4 закрыт.
- Рекомендуемый следующий мандат: Product Distribution / Installers / Release Targets.

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

## Mandate 4 Closure Status

Product UI / Packaging закрыт.

Готово:

- Human-first terminal interface через `mge tui` на `ratatui` + `crossterm`.
- First-run setup helper через `mge setup`.
- TUI setup wizard для первого запуска.
- TUI screens: dashboard, recall, add memory, seal/checkpoint, status/diagnostics/doctor, index benchmark, Markdown export/import status, settings и help.
- Runtime EN/RU language switching через F1, L/l и Д/д.
- Safe encrypted setup guidance через `--passphrase-env`; passphrase читается из environment, а не вводится в TUI.
- Thin `mge-cli` app service layer для TUI и CLI-oriented diagnostics.
- Read-only `mge doctor` diagnostics для store structure, manifest/security state, required files, optional unlock и explicit deep validation.
- Repo-local release build scripts:
  - `scripts/build-release.sh`
  - `scripts/build-release.ps1`
- Repo-local release smoke scripts:
  - `scripts/smoke-release.sh`
  - `scripts/smoke-release.ps1`
- Release scripts выполняют `cargo build -p mge-cli --bins --release`, проверяют release binaries и запускают CLI/encrypted/MCP/SDK smoke checks без публикации packages и без коммита artifacts.
- Windows PowerShell build/smoke scripts проверены на этой машине.
- Linux/macOS `.sh` build/smoke scripts присутствуют и синхронизированы с PowerShell behavior, но локально не запускались, потому что WSL/Linux не установлен.
- CLI, MCP JSON-RPC, Python SDK, TypeScript SDK и Rust example smokes проходят через release smoke script.
- Local encrypted demo workflow scripts:
  - `scripts/demo-local-memory.sh`
  - `scripts/demo-local-memory.ps1`

Намеренно не реализовано:

- Package publishing.
- External MCP SDK dependency.
- Heavy UI framework.
- Web/desktop GUI.
- Storage, codec, filter, recall или encryption format changes.
- Markdown import остаётся disabled; Markdown export поддержан и является plaintext by design.
- Full interactive TUI real-TTY end-to-end automation не реализован; TUI behavior покрыт unit tests, help smokes и manual terminal checks.

## Текущие Ограничения

- Human UI сейчас terminal-first через `mge tui`; web/desktop GUI нет.
- Full interactive real-TTY TUI e2e automation пока нет.
- Vector database нет.
- Encrypted indexes и blind marker metadata пока нет.
- Encrypted Markdown export пока нет.
- Package publishing пока нет.
- Linux/macOS `.sh` release scripts присутствуют, но не проверены локально на этой Windows-машине.
- Markdown import disabled.
- External MCP SDK dependency не добавлена.
- Automatic migration from unencrypted stores to encrypted stores пока нет.
- Большой user-provided corpus всё ещё полезен перед новой performance work.

## Последняя Verification Baseline

Последняя Mandate 4 TUI/package/release проверка:

- `cargo fmt --check`: passed.
- `cargo test`: passed, 157 tests.
- `cargo check -p mge-cli --bins`: passed.
- `cargo build -p mge-cli --bins --release`: passed.
- Release binary TUI help smoke: `mge tui --help` passed.
- Release binary setup help smoke: `mge setup --help` passed.
- CLI quickstart smoke: passed во временном store через release script.
- Encrypted smoke: passed через `MGE_RELEASE_SMOKE_PASSPHRASE`.
- MCP JSON-RPC smoke: passed для `mge_schema` и `mge_stats`.
- Python SDK smoke: passed при запуске из `scripts/smoke-release.ps1`.
- TypeScript SDK smoke: passed при запуске из `scripts/smoke-release.ps1`.
- Rust agent host example smoke: passed при запуске из `scripts/smoke-release.ps1`.
- `scripts/build-release.ps1`: passed.
- `scripts/smoke-release.ps1`: passed.
- POSIX `.sh` release scripts обновлены для Linux/macOS; на этом Windows host они не запускались, потому что WSL не имеет установленного distribution.
- `scripts/demo-local-memory.ps1`: passed.

Mandate 4 закрыл terminal UI, packaging/dev UX, release-script и read-only diagnostics слой. Storage/codec/filter/recall/security formats не менялись.

## Следующий Рекомендуемый Шаг

Начать Mandate 5: Product Distribution / Installers / Release Targets.
