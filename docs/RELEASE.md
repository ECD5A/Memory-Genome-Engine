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

## Install

For normal users, install a published release with mandatory archive checksum verification:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/install-release.ps1
powershell -ExecutionPolicy Bypass -File scripts/install-release.ps1 -Version v0.1.2
```

```bash
./scripts/install-release.sh
./scripts/install-release.sh --version v0.1.2
```

The release installers:

- detect the supported host archive;
- download the archive and combined `SHA256SUMS` from GitHub Releases;
- require an exact SHA-256 match before extraction;
- install only `mge` and `mge-mcp-server` into `~/.local/bin` by default;
- never request admin/root access or modify a shell profile.

`-SourceDirectory` / `--source-dir` installs from a local mirror or fixture while retaining checksum verification. `-BaseUrl` / `--base-url` selects an HTTP mirror. These options are used by release verification and are mutually exclusive.

### Install From A Source Checkout

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

Generated query difficulty can be selected independently:

```bash
--query-profile lexical
--query-profile paraphrase
--query-profile hard-negative
--query-profile mixed
```

The report includes Hit/Recall/Precision@K, MRR@K, nDCG@K, and no-answer accuracy for hard negatives. Paraphrase fixtures retain scope/component anchors; they are not a semantic embedding benchmark. Hard negatives deliberately share a scope with unrelated records. Default retrieval ranks candidates without an automatic semantic abstention threshold, so a non-empty result for such a query must not be interpreted as a verified answer. Agent hosts that prefer silence over weak matches can opt in with `--min-score` / `min_score`.

Generated fixtures are deterministic but synthetic. Local LongMemEval/LoCoMo adapters require user-supplied datasets; results measure retrieval, not final LLM answer quality, and must not be presented as cross-project claims without identical corpora and settings.

## Measured Engineering Baseline

The public baseline was repeated from core commit `2fdbc99` on:

- Intel Core i7-9750H, 6 cores / 12 logical processors;
- Windows 10 x64 (`10.0.19045`);
- Rust 1.95.0;
- optimized release builds;
- AC power with the Windows high-performance plan;
- five timing repeats per query.

Reproduce the deterministic generated retrieval run:

```bash
cargo run --locked --release --manifest-path tools/agent-memory-eval/Cargo.toml -- \
  --profile medium --ingest-mode session-chunk --top-k 5 --repeats 5 \
  --index both --modes focused-broad --baselines all --output json \
  --report target/eval-public-baseline.json
```

Default Exact index results:

| Metric | Result |
|---|---:|
| Memories / queries | 1,280 / 80 |
| Focused Hit@5 / Recall@5 | 1.00 / 1.00 |
| Hot focused recall, avg / p50 / p95 | 0.514 / 0.502 / 0.626 ms |
| Repeated sealed focused recall, avg / p50 / p95 | 0.270 / 0.287 / 0.379 ms |
| Cold focused recall excluding open, avg | 1.818 ms |
| Cold store open + focused recall, avg | 2.404 ms |

A second run used the repository's tracked text/source files as a local corpus performance workload:

```bash
cargo run --locked --release -p mge-cli --bin mge-corpus-bench -- \
  --corpus . --store-root <TEMP_DIR_OUTSIDE_REPOSITORY> \
  --max-files 300 --max-bytes 52428800 --chunk-lines 40 --repeats 5 --seed 1
```

That run imported 1,068,576 bytes into 985 chunks and 46 sealed pages. The Exact store occupied 3,402,654 bytes. Hot focused recall averaged 0.356 ms, repeated sealed focused recall 0.092 ms, cold sealed focused recall 1.838 ms, and repeated locality reduced focused recall latency by 94%.

Interpretation limits:

- The generated workload has deterministic, identifiable relevant records; it verifies retrieval correctness and engine behavior, not open-domain reasoning.
- The published table uses the lexical query profile. Paraphrase and hard-negative profiles are development evidence and must be reported separately rather than blended into that baseline.
- The repository corpus run is a performance workload. Its generated marker-targeted queries are not a real-world retrieval-quality score.
- External-dataset evidence is reported separately below and must not be merged with this deterministic baseline.
- Timing changes with hardware, filesystem state, background load, corpus shape, and ingestion settings.
- BM25, keyword, text-candidate, and page-token rows emitted by the eval harness are algorithmic diagnostics, not complete competing memory products.
- JSON reports under `target/` are generated development artifacts and are not runtime storage or committed release assets.

## External Retrieval Evidence

The following local runs use official public dataset files without bundling or modifying them:

- LongMemEval repository commit `9e0b455f4ef0e2ab8f2e582289761153549043fc`, Oracle dataset revision `98d7416c24c778c2fee6e6f3006e7a073259d48f`, file SHA-256 `821a2034d219ab45846873dd14c14f12cfe7776e73527a483f9dac095d38620c`;
- LongMemEval-S cleaned file SHA-256 `d6f21ea9d60a0d56f34a05b609c79c88a451d2ae03597821ea3d5a9678c3a442`;
- LoCoMo repository commit `3eb6f2c585f5e1699204e3c3bdf7adc5c28cb376`, `data/locomo10.json` SHA-256 `79fa87e90f04081343b8c8debecb80a9a6842b76a7aa537dc9fdf651ea698ff4`.

Quality runs use release builds and strict top-K 5. Oracle and LoCoMo cover both page indexes; the larger LongMemEval-S run uses the default Exact index. Timing is intentionally excluded from these quality tables because it is machine- and load-sensitive.

```bash
cargo run --locked --release --manifest-path tools/agent-memory-eval/Cargo.toml -- \
  --input <longmemeval_oracle.json> --input-format long-mem-eval \
  --ingest-mode session-chunk --top-k 5 --repeats 1 --index both \
  --modes focused-broad --baselines bm25 --output json --report <REPORT.json>

cargo run --locked --release --manifest-path tools/agent-memory-eval/Cargo.toml -- \
  --input <locomo10.json> --input-format locomo \
  --ingest-mode raw-turn --top-k 5 --repeats 1 --index both --modes focused-broad \
  --baselines bm25 --output json --report <REPORT.json>
```

LongMemEval Oracle converted into 4,578 deterministic session chunks and 500 queries, including 30 `_abs` questions treated as negative retrieval cases:

| Retrieval path | Mode | Hit@5 | Recall@5 | MRR@5 | nDCG@5 |
|---|---|---:|---:|---:|---:|
| MGE Exact sealed | focused | 0.972 | 0.877 | 0.775 | 0.761 |
| MGE BinaryFuse sealed | focused | 0.972 | 0.877 | 0.775 | 0.761 |
| MGE Exact sealed | broad | 0.972 | 0.877 | 0.775 | 0.761 |
| MGE BinaryFuse sealed | broad | 0.972 | 0.877 | 0.775 | 0.761 |
| Eval-only BM25 | focused | 0.972 | 0.876 | 0.789 | 0.770 |

LoCoMo converted into 5,881 memories and 1,977 evidence-bearing queries; nine queries without usable evidence annotations were skipped:

| Retrieval path | Mode | Hit@5 | Recall@5 | MRR@5 | nDCG@5 |
|---|---|---:|---:|---:|---:|
| MGE Exact sealed | focused | 0.531 | 0.487 | 0.393 | 0.401 |
| MGE BinaryFuse sealed | focused | 0.531 | 0.487 | 0.393 | 0.401 |
| MGE Exact sealed | broad | 0.544 | 0.499 | 0.402 | 0.411 |
| MGE BinaryFuse sealed | broad | 0.544 | 0.499 | 0.402 | 0.411 |
| Eval-only BM25 | focused | 0.545 | 0.499 | 0.415 | 0.421 |

The LoCoMo adapter also measures ingestion granularity with the production session chunker. On the same file and strict focused top-5 run, the local trade-off was:

| Ingestion | Memories | Hit@5 | Recall@5 | MRR@5 | nDCG@5 | Average context tokens |
|---|---:|---:|---:|---:|---:|---:|
| Raw turn | 5,881 | 0.533 | 0.489 | 0.393 | 0.402 | 295 |
| Session chunk, 2 turns | 3,010 | 0.626 | 0.574 | 0.488 | 0.492 | 462 |
| Session chunk, 4 turns | 1,570 | 0.737 | 0.686 | 0.575 | 0.586 | 788 |
| Session chunk, 8 turns | 848 | 0.797 | 0.747 | 0.640 | 0.649 | 1,407 |
| Whole session | 272 | 0.861 | 0.810 | 0.698 | 0.710 | 3,715 |

This is a granularity trade-off inside MGE, not a competitor comparison. Whole-session ingestion scores highest but returns much larger context. Four-turn chunks are the measured compact recommendation; the production default remains eight turns because it preserves the quality-first behavior. Override it in the eval harness with `--session-chunk-max-turns 4`; `--session-chunk-max-bytes` controls the independent byte cap.

LongMemEval-S converted into 85,253 session chunks and 500 queries:

| Retrieval path | Mode | Hit@5 | Recall@5 | MRR@5 | nDCG@5 |
|---|---|---:|---:|---:|---:|
| MGE Exact sealed | focused | 0.896 | 0.782 | 0.701 | 0.676 |
| MGE Exact sealed | broad | 0.898 | 0.784 | 0.701 | 0.677 |
| Eval-only BM25 | focused | 0.894 | 0.785 | 0.717 | 0.688 |

Broad recall keeps wider candidate selection but now treats `max_items` as a strict output budget. With `max_items=5`, the output-only change preserved strict top-5 quality while reducing average returned context:

| Dataset | Previous items / estimated tokens | Strict budget items / estimated tokens | Estimated token reduction |
|---|---:|---:|---:|
| LongMemEval Oracle | 8.92 / 6,720 | 4.57 / 3,535 | 47% |
| LoCoMo | 20.00 / 1,083 | 5.00 / 300 | 72% |

These are retrieval-adapter results, not official end-to-end LongMemEval or LoCoMo answer scores. The BM25 row is an in-memory algorithmic diagnostic and is not a product-level latency comparison. Exact and BinaryFuse remain quality-equivalent in these runs; BinaryFuse stays optional rather than being presented as a universal speed improvement.

Dev-only candidate reranking experiments tested global BM25, candidate-local BM25, equal RRF, and weighted RRF. Pure BM25 and weighted/local variants were rejected after dataset regressions. Equal RRF between the existing sealed ranking and production-token global BM25 improved all four reported metrics on Oracle, LoCoMo, and LongMemEval-S, so that narrow sealed-only variant was promoted. Its document-frequency statistics are a rebuildable runtime index; hot and mixed hot/sealed ranking remain unchanged.

The current production contract returns ranked candidates rather than asserting that a question is answerable. A post-hoc score-threshold sweep is therefore reported only as a design diagnostic. On the deterministic mixed fixture it reached 0.975 balanced accuracy, but on strict-top-K LongMemEval sealed broad it reached only 0.668 (0.602 positive hit/accept rate and 0.733 negative rejection). The threshold was selected and measured on the same samples, not a holdout set. This is insufficient evidence for a production threshold, so recall behavior remains unchanged.

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
- `cargo audit` reports no known vulnerable runtime dependencies.
- `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps` passes.
- `cargo check --locked -p mge-cli --bins` passes.
- `cargo build --locked -p mge-cli --bin mge --bin mge-mcp-server --release` passes.
- The CI MSRV job passes on Rust 1.95.
- `scripts/build-release.sh` or `scripts/build-release.ps1` passes and creates a local archive plus `SHA256SUMS`.
- `scripts/smoke-release.sh` or `scripts/smoke-release.ps1` passes.
- `scripts/install-release.sh` or `scripts/install-release.ps1` rejects a tampered archive and installs a verified release into a user-writable directory.
- `scripts/install.sh` or `scripts/install.ps1` installs a locally built source checkout into a user-writable directory.
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
- Release installers verify GitHub or mirror archives before installing; source installers copy locally built binaries into a user-writable directory.
- Windows and WSL Ubuntu release paths are locally verified. macOS remains enabled in CI and release automation; its result must be taken from the GitHub-hosted macOS job because no local macOS host is available.
- No external MCP SDK dependency is bundled.
- Python and TypeScript packages are repository-local developer wrappers.
- Release artifacts should be generated from the Rust workspace, not from copied binaries.
- Local archives under `target/mge-release/archives/` are generated artifacts and must not be committed.

## Tag Release Workflow

`.github/workflows/release.yml` runs only for `v*` tags. It verifies format, workspace and eval tests, strict clippy, rustdoc, and Rust 1.95 compatibility with the locked dependency graph. It then builds checksummed product archives for Windows x86-64, Linux x86-64, macOS Apple Silicon, and macOS Intel, uploads them as workflow artifacts, and creates or updates a **draft** GitHub Release with one combined `SHA256SUMS`. The workflow includes only `mge` and `mge-mcp-server`; SDK packages and development benchmark binaries are not published. A maintainer must review checksums, notes, and every platform result before publishing the draft.

Rust crates, the development eval harness, and both repository-local SDK manifests use version `0.1.2`. Integration schema version `4` is independent from package versioning.

## Package Publishing Plan

Recommended order:

1. Keep GitHub release archives as the primary distribution path for preview releases.
2. Keep `cargo install --git https://github.com/ECD5A/Memory-Genome-Engine.git --bin mge` as a developer path after each release candidate is tagged.
3. Add Scoop later for Windows if release archives prove stable.
4. Add Homebrew later after the hosted macOS build/smoke and user feedback establish a stable installation path.
5. Treat PyPI and npm as separate thin-SDK packaging work; do not publish them until the CLI binary discovery/install story is clear.

Do not publish packages from this repository until release ownership, versioning, and rollback rules are explicit. Do not introduce admin/root installer flows for preview releases.

Current recommendation: GitHub release assets are enough for the public preview. Package-manager publishing should wait until Windows, Linux, and macOS preview users have exercised the archives.

## GitHub v0.1.2 Release

Create the next public preview `v0.1.2` from a clean `main` commit after the checklist above passes. The existing `v0.1.0` tag remains immutable. Use `v0.1.2-rc.1` first to exercise the complete private tag workflow, cross-platform installer gates, and exact release archives.

Recommended assets:

- `mge-windows-x86_64.zip`;
- `mge-linux-x86_64.tar.gz`;
- `mge-macos-aarch64.tar.gz`;
- `mge-macos-x86_64.tar.gz`;
- combined `SHA256SUMS`.

Keep the release product-focused:

- include `mge` and `mge-mcp-server`;
- do not include development benchmark binaries unless the release is explicitly marked as a development/tooling release;
- do not upload generated stores, logs, passphrases, private corpus data, or `target/` directories;
- state that macOS is a supported CI/release target but was not executed locally on this Windows development host.

Tag command shape:

```bash
git tag -a v0.1.2 -m "Memory Genome Engine v0.1.2"
git push origin v0.1.2
```

The tag workflow creates the draft and uploads all platform archives. Do not publish it until the downloaded assets, checksums, release notes, and platform jobs are reviewed.
