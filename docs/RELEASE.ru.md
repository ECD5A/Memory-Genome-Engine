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

Repo-local build helpers:

```bash
./scripts/build-release.sh
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Scripts собирают local binaries и проверяют наличие expected executables. Они не публикуют packages и не коммитят artifacts.

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
cargo run -p mge-cli -- setup --help
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

Smoke scripts запускают local CLI, encrypted store, MCP, SDK и Rust example checks там, где доступен нужный local toolchain. Optional Python/Node/rustc checks пропускаются с сообщением, если toolchain недоступен.

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

## Benchmark Workflows

`mge-synthetic-bench` нужен для повторяемой проверки exact-vs-BinaryFuse на generated memory cells:

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

`mge-corpus-bench` нужен для local real-workload measurement:

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

Safety rules для corpus benchmark:

- читать только local text/code files;
- пропускать unsupported binary extensions;
- пропускать symlinks;
- не исполнять corpus files;
- не устанавливать dependencies;
- не менять corpus files;
- писать generated stores только в `--store-root`.

Как читать report:

- `hot`: recall до seal из L1 Hot RAM.
- `sealed cold`: открывает store для каждого query; включает open/recovery, page read/decode, filtering, ranking и `ContextPacket` build.
- `sealed repeated`: использует один engine instance; показывает decoded page cache и runtime scoring cache locality.
- `locality benefit`: насколько repeated sealed recall быстрее cold sealed recall.
- `page decode share`: доля repeated focused recall, уходящая на decode loaded sealed pages.
- `scoring/filtering share`: inclusive bottleneck signal для cell filtering и scoring.
- `ContextPacket share`: стоимость построения returned memory items и debug details.

`ExactMarkerPageIndex` - default baseline. `BinaryFusePageIndex` - optional probabilistic backend: extra candidate pages допустимы, но exact candidates не должны теряться при корректно построенных filters.

Не начинать custom page codec только потому, что в stack есть MessagePack. Custom codec оправдан только если большой real corpus показывает, что page decode стабильно доминирует в repeated sealed recall, а более простые cache/policy changes проблему не решают.

## Release Checklist

- Git working tree clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- CLI smoke passes.
- TUI help smoke (`mge tui --help`) passes.
- Setup help smoke (`mge setup --help`) passes.
- Encrypted smoke passes, если менялись security docs или encrypted storage.
- MCP/SDK smoke passes, если менялись integration docs или wrappers.
- `mge doctor --deep` проходит для unencrypted и encrypted smoke stores.
- README, Quickstart, Security, Integration, Release и Project Status links актуальны.
- Secret material не закоммичен.
- `LICENSE` есть и MIT.

## Current Publishing Policy

- Package publishing пока не автоматизирован.
- External MCP SDK dependency не bundled.
- Python и TypeScript packages - repository-local developer wrappers.
- Release artifacts должны собираться из Rust workspace, а не из copied binaries.
