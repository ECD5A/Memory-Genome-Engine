# Memory Genome Engine - Project Status

[Русская версия](PROJECT_STATUS.ru.md)

This file is the working ledger for this repository. Keep it current so we do not repeat completed work or reopen decisions without a reason.

## Current Focus

- Build v0.1 Rust-first core and CLI in this folder.
- Keep the first implementation deterministic, local, marker/page based, and ready for later compression, encryption, SDKs, and MCP.

## Done

- Repository initialized with git.
- Rust toolchain confirmed: `cargo 1.95.0`, `rustc 1.95.0`.
- Initial workspace structure selected:
  - `crates/mge-core`
  - `crates/mge-cli`
  - `docs`
  - `examples`
  - `tests`
- Rust workspace created.
- `mge-core` implemented with:
  - typed memory models;
  - marker canonicalization and persistent dictionary;
  - deterministic marker extraction;
  - append-only hot JSONL store;
  - page model and JSON page codec;
  - exact marker-to-page candidate index;
  - recall, reranking, filtering, and context packet output;
  - extension traits for store, page codec, compression, index, retrieval, and security.
- `mge-cli` implemented with:
  - `init`
  - `remember`
  - `recall`
  - `seal`
  - `inspect`
  - `stats`
  - `export --format json`
- Documentation added:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/ROADMAP.md`
  - `examples/basic_usage.md`
- Rust tests added for marker canonicalization, dictionary IDs, cell creation, marker generation, hot recall, sealing, sealed recall, index lookup, filtering, context packet text, and stats output.
- MIT license added for ECD5A.
- README polished with badges, navigation, Donate block, and license section.
- Russian mirrors added for important Markdown files.
- `MessagePackPageCodec` added behind the existing `PageCodec` trait as the first v0.2 codec step.
- `ZstdCompression` added behind the existing `Compressor` trait.
- Store manifest now records default `page_codec` and `compression` for newly sealed pages.
- Page catalog entries now record per-page codec/compression for mixed-store and backward-compatible reads.
- CLI `init` now supports `--page-codec json|messagepack` and `--compression none|zstd`.
- CLI `config show` and `config set` added for existing stores.
- Storage config updates change only future seal defaults; existing pages remain untouched and readable through catalog metadata.
- Tests added for zstd roundtrip, init options, MessagePack+zstd sealed recall, and legacy catalog defaults.
- `PageClusterer` trait added.
- `ScopeKindClusterer` kept as the default seal clustering strategy.
- `MarkerOverlapClusterer` added as a deterministic no-ML extension strategy.
- `PageBuildOptions` added with 64 KiB target page bytes and 512 max cells defaults.
- Page builder now enforces logical page limits.

## In Progress

- No active implementation item at this moment.

## Next

- Add store-level clustering config and optional marker-overlap seal mode.
- Add a real static filter index implementation behind `CandidatePageIndex`.
- Add SDK wrappers only after the Rust core API stabilizes.

## Rollbacks / Do Not Repeat

- Do not start with UI, chatbot, cloud service, vector DB, fake encryption, fake Binary Fuse, or Markdown as the internal storage format.
- Do not store real credentials or secrets. Sensitive values must be represented with `SecretReference` metadata/placeholders.
- Do not replace the marker/page API with a vector-only retrieval flow.

## Verification Commands

```bash
cargo build
cargo test
cargo run -p mge-cli -- init
cargo run -p mge-cli -- remember "User prefers concise technical explanations" --kind user_preference --scope global --trust user_confirmed
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
```

## Verification Status

- `cargo fmt`: passed.
- `cargo test`: passed, 22 tests.
- Milestone smoke commands: passed.
- MessagePack+zstd smoke commands: passed.
- Config show/set mixed-store smoke commands: passed.
- Default clustering smoke commands: passed.
- Smoke result after sealing:
  - hot cells: 0
  - sealed pages: 1-2 depending on smoke scenario
  - sealed cells: 1-2 depending on smoke scenario
  - index type: `exact_marker_page_index`
  - current page codec: `messagepack`
  - current compression: `zstd`
