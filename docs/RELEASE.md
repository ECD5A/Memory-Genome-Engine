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
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- validate --deep
```

## Encrypted Smoke

```bash
export MGE_PASSPHRASE="use-a-real-secret"
cargo run -p mge-cli -- init --encrypted --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- remember "private release smoke" --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- checkpoint --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- seal --passphrase-env MGE_PASSPHRASE
cargo run -p mge-cli -- recall "private release smoke" --passphrase-env MGE_PASSPHRASE
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

## Release Checklist

- Git working tree is clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- CLI smoke passes.
- Encrypted smoke passes if security docs or encrypted storage changed.
- MCP/SDK smoke passes if integration docs or wrappers changed.
- README, Quickstart, Security, Integration, Benchmarks, and Project Status links are current.
- No secret material is committed.
- `LICENSE` is present and MIT.

## Current Publishing Policy

- No package publishing is automated yet.
- No external MCP SDK dependency is bundled.
- Python and TypeScript packages are repository-local developer wrappers.
- Release artifacts should be generated from the Rust workspace, not from copied binaries.
