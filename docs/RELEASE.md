# Release

[Russian version](RELEASE.ru.md)

This document is for local build, packaging, and release readiness checks. It does not define runtime storage behavior.

## Build

```bash
cargo build --release -p mge-cli --bins
```

Release binaries:

```text
target/release/mge
target/release/mge-mcp-server
target/release/mge-synthetic-bench
target/release/mge-corpus-bench
```

On Windows the files have `.exe` suffixes.

Repo-local build helpers:

```bash
./scripts/build-release.sh
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

The scripts build local binaries and verify that expected executables exist. They do not publish packages or commit artifacts.

## Test

```bash
cargo fmt --check
cargo test
```

Run focused integration smokes when changing MCP/SDK packaging:

```bash
cargo test -p mge-cli --test cli_smoke mcp
cargo test -p mge-cli --test cli_smoke encrypted_sealed_recall_smoke
cargo test -p mge-cli --test cli_smoke rust_agent_host_cli_example_smoke -- --exact
```

## CLI Smoke

```bash
cargo run -p mge-cli -- init --profile fast
cargo run -p mge-cli -- remember "release smoke memory" --kind project_fact --scope release --trust tool_observed
cargo run -p mge-cli -- recall "release smoke"
cargo run -p mge-cli -- doctor --store .memory-genome --deep
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- validate --deep
cargo run -p mge-cli -- tui --help
```

Repo-local smoke helpers:

```bash
./scripts/smoke-release.sh
powershell -ExecutionPolicy Bypass -File scripts/smoke-release.ps1
```

The smoke scripts run local CLI, encrypted store, MCP, SDK, and Rust example checks where the required local toolchain is available. Optional Python/Node/rustc checks are skipped with a message when unavailable.

## Encrypted Smoke

```bash
export MGE_PASSPHRASE="use-a-real-secret"
cargo run -p mge-cli -- init --encrypted --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- remember "private release smoke" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- checkpoint --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- seal --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- recall "private release smoke" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- doctor --store .memory-genome --deep --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- validate --deep --passphrase-env MGE_PASSPHRASE
```

## SDK Packaging Checks

Python SDK is a thin wrapper over the Rust CLI. Do not publish from this repository until release ownership is explicit.

```bash
python -m pip install -e sdk/python
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
python examples/python_basic_usage.py
```

Optional wheel metadata check:

```bash
python -m pip wheel --no-deps --no-build-isolation sdk/python
```

TypeScript SDK is also a thin wrapper over the Rust CLI:

```bash
cd sdk/typescript
npm run smoke
npm run check
```

`npm run check` requires a local TypeScript toolchain.

## Benchmark Smoke

Run benchmark smoke only when benchmark output or performance-related code changes:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 300 --pages 30 --repeats 2 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-release-bench --repeats 1 --seed 1
```

Benchmark JSON is report/debug output only, not runtime storage.

## Benchmark Workflows

Use `mge-synthetic-bench` for repeatable exact-vs-BinaryFuse checks on generated memory cells:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- \
  --cells 1200 \
  --pages 120 \
  --scopes 16 \
  --markers-per-cell 5 \
  --marker-groups 12 \
  --targeted-queries 6 \
  --noise-queries 3 \
  --repeats 5 \
  --seed 1
```

Use `mge-corpus-bench` for local real-workload measurement:

```bash
cargo run -p mge-cli --bin mge-corpus-bench -- \
  --corpus <LOCAL_CORPUS_DIR> \
  --store-root <SAFE_TEMP_STORE_ROOT> \
  --profile medium \
  --max-files 300 \
  --max-bytes 52428800 \
  --chunk-lines 40 \
  --repeats 3 \
  --seed 1
```

Generated corpus profiles:

```bash
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-bench-small --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile medium --store-root ../mge-bench-medium --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile code-heavy --store-root ../mge-bench-code --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile docs-heavy --store-root ../mge-bench-docs --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile mixed --store-root ../mge-bench-mixed --seed 1
```

Corpus benchmark safety rules:

- read local text/code files only;
- skip unsupported binary extensions;
- skip symlinks;
- never execute corpus files;
- never install dependencies;
- never modify corpus files;
- write generated stores only under `--store-root`.

Report terms:

- `hot`: recall before sealing from L1 Hot RAM.
- `sealed cold`: opens the store for each query; includes open/recovery, page read/decode, filtering, ranking, and `ContextPacket` build.
- `sealed repeated`: reuses one engine instance; shows decoded page cache and runtime scoring cache locality.
- `locality benefit`: how much faster repeated sealed recall is than cold sealed recall.
- `page decode share`: repeated focused recall time spent decoding loaded sealed pages.
- `scoring/filtering share`: inclusive bottleneck signal for cell filtering and scoring.
- `ContextPacket share`: cost of building returned memory items and debug details.

`ExactMarkerPageIndex` is the default baseline. `BinaryFusePageIndex` is optional and probabilistic: extra candidate pages are allowed, but exact candidates must not be missed when filters are built correctly.

Do not start custom page codec work just because MessagePack is present. A custom codec is only justified if a large real corpus shows page decode dominating repeated sealed recall and simpler cache/policy changes do not address it.

## Release Checklist

- Git working tree is clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- CLI smoke passes.
- TUI help smoke (`mge tui --help`) passes.
- Encrypted smoke passes if security docs or encrypted storage changed.
- MCP/SDK smoke passes if integration docs or wrappers changed.
- `mge doctor --deep` passes for unencrypted and encrypted smoke stores.
- README, Quickstart, Security, Integration, Release, and Project Status links are current.
- No secret material is committed.
- `LICENSE` is present and MIT.

## Current Publishing Policy

- No package publishing is automated yet.
- No external MCP SDK dependency is bundled.
- Python and TypeScript packages are repository-local developer wrappers.
- Release artifacts should be generated from the Rust workspace, not from copied binaries.
