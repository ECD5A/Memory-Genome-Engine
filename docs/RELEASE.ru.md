# Release

[English version](RELEASE.md)

Этот документ описывает local build, packaging и release readiness checks. Он не определяет runtime storage behavior.

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

На Windows файлы имеют `.exe` suffix.

## Test

```bash
cargo fmt --check
cargo test
```

Focused integration smokes при изменении MCP/SDK packaging:

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

Python SDK - thin wrapper поверх Rust CLI. Не публиковать package из этого репозитория, пока release ownership не зафиксирован явно.

```bash
python -m pip install -e sdk/python
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
python examples/python_basic_usage.py
```

Optional wheel metadata check:

```bash
python -m pip wheel --no-deps --no-build-isolation sdk/python
```

TypeScript SDK также thin wrapper поверх Rust CLI:

```bash
cd sdk/typescript
npm run smoke
npm run check
```

`npm run check` требует local TypeScript toolchain.

## Benchmark Smoke

Запускать benchmark smoke только если менялся benchmark output или performance-related code:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 300 --pages 30 --repeats 2 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-release-bench --repeats 1 --seed 1
```

Benchmark JSON - только report/debug output, не runtime storage.

## Release Checklist

- Git working tree clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- CLI smoke passes.
- Encrypted smoke passes, если менялись security docs или encrypted storage.
- MCP/SDK smoke passes, если менялись integration docs или wrappers.
- README, Quickstart, Security, Integration, Benchmarks и Project Status links актуальны.
- Secret material не закоммичен.
- `LICENSE` есть и MIT.

## Current Publishing Policy

- Package publishing пока не автоматизирован.
- External MCP SDK dependency не bundled.
- Python и TypeScript packages - repository-local developer wrappers.
- Release artifacts должны собираться из Rust workspace, а не из copied binaries.
