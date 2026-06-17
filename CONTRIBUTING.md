# Contributing

Memory Genome Engine is a local-first Rust project. Keep changes small, measurable, and compatible with the existing architecture.

## Before Opening A Pull Request

- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo check -p mge-cli --bins` when CLI, MCP, TUI, release, or SDK-facing code changes.
- Run `scripts/smoke-release.ps1` on Windows or `scripts/smoke-release.sh` on Linux/macOS when release packaging changes.
- Do not commit binaries, generated stores, target directories, passphrases, `.env` files, or private corpus data.

## Architecture Rules

- Do not rewrite storage layout, page codec, recall semantics, encryption format, or candidate index API without a design note and maintainer agreement.
- Do not add new filter families unless benchmark evidence shows a real need.
- JSON/JSONL may be protocol, debug, or benchmark output only; it is not runtime storage.
- `MemoryCell.markers` remains a flattened runtime/index compatibility view.

## Documentation Rules

Keep Markdown consolidated:

- short product entry: `README.md` / `README.ru.md`;
- quick commands: `QUICKSTART.md`;
- architecture, security, integration, and release details under `docs/`;
- keep Russian public documentation limited to `README.ru.md` unless a new mirror is explicitly approved.

Do not recreate removed long-form docs such as `docs/MCP.md`, `docs/SDK.md`, `docs/BENCHMARKS.md`, `docs/ROADMAP.md`, `examples/basic_usage.md`, or `examples/agent_workflow.md`.

## Pull Request Style

- Explain the user-visible behavior change.
- List tests and smokes run.
- Call out any skipped platform checks.
- Keep generated benchmark output out of the repository unless it is a small intentional fixture.
