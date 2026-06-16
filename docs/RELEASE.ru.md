# Release

[English version](RELEASE.md)

Этот документ описывает local build, packaging и release readiness checks. Он не определяет runtime storage behavior.

## Build

```bash
cargo build --release -p mge-cli --bin mge --bin mge-mcp-server
```

Product release binaries:

```text
target/release/mge
target/release/mge-mcp-server
```

На Windows файлы имеют `.exe` suffix. Development-only benchmark binaries остаются buildable из workspace, но не входят в default product build, install, smoke или release layout.

Repo-local build helpers:

```bash
./scripts/build-release.sh
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

Scripts собирают product release binaries, проверяют product executables и готовят product release layout:

```text
target/mge-release/<platform>/
  bin/
  docs/
```

Они учитывают `CARGO_TARGET_DIR`, если он задан. Они не публикуют packages, не создают tracked `dist/` и не коммитят artifacts.

Development benchmark tools можно собрать и скопировать в отдельную папку `dev-tools/` только явно:

```bash
MGE_INCLUDE_DEV_TOOLS=1 ./scripts/build-release.sh
```

```powershell
$env:MGE_INCLUDE_DEV_TOOLS = "1"
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

## Platform Verification

- Windows PowerShell scripts локально проверены на текущей Windows-машине.
- Linux shell scripts локально проверены через WSL Ubuntu.
- macOS shell scripts идут по тому же POSIX path, но macOS локально не проверялся с этой Windows-машины.

## Install From Source

Установить local release binaries в user-writable directory:

```bash
./scripts/install.sh --install-dir "$HOME/.local/bin"
powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -InstallDir "$env:USERPROFILE\.local\bin"
```

Install scripts собирают product release binaries, если не передан `--no-build` / `-NoBuild`, затем копируют только product binaries:

- `mge`
- `mge-mcp-server`

Они не публикуют packages, не требуют admin/root privileges и не меняют shell profile files. Добавьте install directory в `PATH` вручную, если нужно.

Development benchmark tools устанавливаются только при явном opt-in:

```bash
./scripts/install.sh --include-dev-tools
powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -IncludeDevTools
```

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

Smoke scripts собирают product binaries, затем запускают release binaries из `target/release` или `$CARGO_TARGET_DIR/release`.

Они проверяют:

- product release binaries существуют;
- `mge --version`;
- `mge tui --help`;
- `mge setup --help`;
- unencrypted CLI workflow во временном store;
- encrypted workflow через `MGE_RELEASE_SMOKE_PASSPHRASE`;
- MCP JSON-RPC schema и `mge_stats`;
- Python SDK example, если доступен `python`;
- TypeScript SDK example, если `node` может его выполнить;
- Rust agent host example, если доступен `rustc`.

Optional Python/Node/rustc checks пропускаются с сообщением, если toolchain недоступен. Scripts пишут stores только во временную папку и удаляют её, если `KEEP_MGE_SMOKE=1` не задан.

Development benchmark tools проверяются release smoke только при явном opt-in:

```bash
MGE_CHECK_DEV_TOOLS=1 ./scripts/smoke-release.sh
```

```powershell
$env:MGE_CHECK_DEV_TOOLS = "1"
powershell -ExecutionPolicy Bypass -File scripts/smoke-release.ps1
```

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

## Development-Only Benchmark Tools

`mge-synthetic-bench` и `mge-corpus-bench` - внутренние development tools для измерения core changes. Они остаются в репозитории для regression checks и будущей performance work, но не входят в default user-facing product surface, install path или release layout.

## Benchmark Smoke

Запускать benchmark smoke только если менялся benchmark output или performance-related code:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 300 --pages 30 --repeats 2 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-release-bench --repeats 1 --seed 1
```

Benchmark JSON - только report/debug output, не runtime storage.

Для более глубоких development-only options используйте `--help` у benchmark binary. Corpus benchmark должен читать только local text/code files, пропускать binary files и unsafe symlinks, не исполнять corpus files, не устанавливать dependencies, не менять corpus files и писать generated stores только в явно заданный safe `--store-root`.

## Release Checklist

- Git working tree clean.
- `cargo fmt --check` passes.
- `cargo test` passes.
- `cargo check -p mge-cli --bins` passes.
- `cargo build -p mge-cli --bin mge --bin mge-mcp-server --release` passes.
- `scripts/build-release.sh` или `scripts/build-release.ps1` passes.
- `scripts/smoke-release.sh` или `scripts/smoke-release.ps1` passes.
- `scripts/install.sh` или `scripts/install.ps1` устанавливает binaries в user-writable directory.
- CLI smoke passes во временном store.
- TUI help smoke (`mge tui --help`) passes.
- Setup help smoke (`mge setup --help`) passes.
- Encrypted smoke passes через passphrase environment variable.
- MCP JSON-RPC smoke passes.
- Python/TypeScript SDK smoke passes, если доступен local toolchain.
- `mge doctor --deep` проходит для unencrypted и encrypted smoke stores.
- README, Quickstart, Security, Integration и Release links актуальны.
- Secret material не закоммичен.
- `LICENSE` есть и Apache-2.0.

## Current Publishing Policy

- Package publishing пока не автоматизирован.
- Install scripts только копируют локально собранные binaries в user-writable directory.
- Windows и WSL Ubuntu release paths локально проверены; macOS всё ещё требует macOS host перед claim full macOS release support.
- External MCP SDK dependency не bundled.
- Python и TypeScript packages - repository-local developer wrappers.
- Release artifacts должны собираться из Rust workspace, а не из copied binaries.
