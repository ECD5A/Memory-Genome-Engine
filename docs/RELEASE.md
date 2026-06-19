# Release

This document is for local build, packaging, and release readiness checks. It does not define runtime storage behavior.

## Build

```bash
cargo build --locked --release -p mge-cli --bin mge --bin mge-mcp-server
```

Product release binaries:

```text
target/release/mge
target/release/mge-mcp-server
```

On Windows the files have `.exe` suffixes. Development-only benchmark binaries remain buildable from the workspace, but they are not part of the default product build, install, smoke, or release layout.

Repo-local build helpers:

```bash
./scripts/build-release.sh
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

The scripts build product release binaries, verify the product executables exist, and prepare a product release layout under:

```text
target/mge-release/<platform>/
  bin/
  docs/
target/mge-release/archives/
  mge-<platform>.zip or mge-<platform>.tar.gz
  SHA256SUMS
```

They honor `CARGO_TARGET_DIR` when it is set. They do not publish packages, create a tracked `dist/` directory, or commit artifacts.

Development benchmark tools can be built and copied into a separate `dev-tools/` folder only when explicitly requested:

```bash
MGE_INCLUDE_DEV_TOOLS=1 ./scripts/build-release.sh
```

```powershell
$env:MGE_INCLUDE_DEV_TOOLS = "1"
powershell -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

## Platform Verification

- Windows PowerShell scripts are locally verified on the current Windows host.
- Linux shell scripts, including the standard MCP smoke revision, are locally verified through WSL2 Ubuntu with Rust 1.96.0 and Bash 5.3.9.
- macOS remains a supported release target and runs the same POSIX build/smoke path on the GitHub-hosted macOS runner. No local macOS execution is claimed because this development machine does not run macOS.

## Install From Source

Install local release binaries into a user-writable directory:

```bash
./scripts/install.sh --install-dir "$HOME/.local/bin"
powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -InstallDir "$env:USERPROFILE\.local\bin"
```

The install scripts build product release binaries unless `--no-build` / `-NoBuild` is passed, then copy product binaries only:

- `mge`
- `mge-mcp-server`

They do not publish packages, require admin/root privileges, or modify shell profile files. Add the install directory to `PATH` manually when needed.

Development benchmark tools are installable only with an explicit opt-in:

```bash
./scripts/install.sh --include-dev-tools
powershell -ExecutionPolicy Bypass -File scripts/install.ps1 -IncludeDevTools
```

## Test

```bash
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo clippy --manifest-path tools/agent-memory-eval/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path tools/agent-memory-eval/Cargo.toml
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

The smoke scripts build product binaries, then run release binaries from `target/release` or `$CARGO_TARGET_DIR/release`.

They check:

- product release binaries exist;
- `mge --version`;
- `mge tui --help`;
- `mge setup --help`;
- unencrypted CLI workflow on a temporary store;
- encrypted workflow through `MGE_RELEASE_SMOKE_PASSPHRASE`;
- MCP JSON-RPC schema and `mge_stats`;
- Python SDK example when `python` is available;
- TypeScript SDK example when `node` can run it;
- Rust agent host example when `rustc` is available.

Optional Python/Node/rustc checks are skipped with a message when unavailable. The scripts write stores only under a temporary directory and remove it unless `KEEP_MGE_SMOKE=1` is set.

Development benchmark tools are checked by release smoke only when explicitly requested:

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

## Development-Only Benchmark Tools

`mge-synthetic-bench` and `mge-corpus-bench` are internal development tools for measuring core changes. They are kept in the repository for regression checks and future performance work, but they are not part of the default user-facing product surface, install path, or release layout.

The separate eval harness compares MGE with keyword/BM25/text-candidate/page-token baselines and reports recall-only versus store-open-inclusive cold latency:

```bash
cargo run --release --manifest-path tools/agent-memory-eval/Cargo.toml -- \
  --profile medium --ingest-mode session-chunk --top-k 5 --repeats 3 \
  --index both --modes focused-broad --baselines all --output text
```

Generated fixtures are deterministic but synthetic. Local LongMemEval/LoCoMo adapters require user-supplied datasets; results measure retrieval, not final LLM answer quality, and must not be presented as cross-project claims without identical corpora and settings.

## Benchmark Smoke

Run benchmark smoke only when benchmark output or performance-related code changes:

```bash
cargo run -p mge-cli --bin mge-synthetic-bench -- --cells 300 --pages 30 --repeats 2 --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-release-bench --repeats 1 --seed 1
```

Benchmark JSON is report/debug output only, not runtime storage.

Use `--help` on either benchmark binary for deeper development-only options. Corpus benchmark runs must read local text/code files only, skip binary files and unsafe symlinks, never execute corpus files, never install dependencies, never modify corpus files, and write generated stores only under an explicit safe `--store-root`.

## Release Checklist

- Git working tree is clean.
- `cargo fmt --check` passes.
- `cargo test --locked` passes.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` passes.
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps` passes.
- `cargo check --locked -p mge-cli --bins` passes.
- `cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server --release` passes.
- The CI MSRV job passes on Rust 1.95.
- `scripts/build-release.sh` or `scripts/build-release.ps1` passes and creates a local archive plus `SHA256SUMS`.
- `scripts/smoke-release.sh` or `scripts/smoke-release.ps1` passes.
- `scripts/install.sh` or `scripts/install.ps1` installs into a user-writable directory.
- CLI smoke passes on a temporary store.
- TUI help smoke (`mge tui --help`) passes.
- Setup help smoke (`mge setup --help`) passes.
- Encrypted smoke passes through a passphrase environment variable.
- MCP JSON-RPC smoke passes.
- Python/TypeScript SDK smoke passes when the local toolchain is available.
- `mge doctor --deep` passes for unencrypted and encrypted smoke stores.
- README, Quickstart, Security, Integration, and Release links are current.
- No secret material is committed.
- `LICENSE` is present and Apache-2.0.

## Current Publishing Policy

- No package publishing is automated yet.
- Install scripts only copy locally built binaries into a user-writable directory.
- Windows and WSL Ubuntu release paths are locally verified. macOS remains enabled in CI and release automation; its result must be taken from the GitHub-hosted macOS job because no local macOS host is available.
- No external MCP SDK dependency is bundled.
- Python and TypeScript packages are repository-local developer wrappers.
- Release artifacts should be generated from the Rust workspace, not from copied binaries.
- Local archives under `target/mge-release/archives/` are generated artifacts and must not be committed.

## Tag Release Workflow

`.github/workflows/release.yml` runs only for `v*` tags. It first verifies format, workspace tests, and strict clippy with the locked dependency graph. It then builds checksummed product archives for Windows x86-64, Linux x86-64, macOS Apple Silicon, and macOS Intel, uploads them as workflow artifacts, and creates or updates a **draft** GitHub Release. The workflow includes only `mge` and `mge-mcp-server`; SDK packages and development benchmark binaries are not published. A maintainer must review checksums, notes, and every platform result before publishing the draft.

Rust crates and both repository-local SDK manifests use version `0.1.0`. Integration schema versioning is independent from package versioning.

## Package Publishing Plan

Recommended order:

1. Keep GitHub release archives as the primary distribution path for preview releases.
2. Keep `cargo install --git https://github.com/ECD5A/Memory-Genome-Engine.git --bin mge` as a developer path after each release candidate is tagged.
3. Add Scoop later for Windows if release archives prove stable.
4. Add Homebrew later after the hosted macOS build/smoke and user feedback establish a stable installation path.
5. Treat PyPI and npm as separate thin-SDK packaging work; do not publish them until the CLI binary discovery/install story is clear.

Do not publish packages from this repository until release ownership, versioning, and rollback rules are explicit. Do not introduce admin/root installer flows for preview releases.

Current recommendation: GitHub release assets are enough for the public preview. Package-manager publishing should wait until Windows, Linux, and macOS preview users have exercised the archives.

## GitHub Preview Release

For a first public preview, create a draft release from a clean `main` commit after the checklist above passes.

Recommended assets:

- Windows archive from `scripts/build-release.ps1`;
- Linux archive from `scripts/build-release.sh`;
- combined `SHA256SUMS`.

Keep the release product-focused:

- include `mge` and `mge-mcp-server`;
- do not include development benchmark binaries unless the release is explicitly marked as a development/tooling release;
- do not upload generated stores, logs, passphrases, private corpus data, or `target/` directories;
- state that macOS is a supported CI/release target but was not executed locally on this Windows development host.

Draft release command shape:

```bash
gh release create v0.1.0-preview --draft \
  --title "v0.1.0-preview" \
  --notes-file release-notes.md \
  target/mge-release/archives/mge-windows-x64.zip \
  target/mge-release/archives/mge-linux-x86_64.tar.gz \
  target/mge-release/archives/SHA256SUMS
```

Do not publish the draft until the uploaded assets and checksums are reviewed.
