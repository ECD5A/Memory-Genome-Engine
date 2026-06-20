# Contributing

Memory Genome Engine is a local-first Rust project. Keep changes small, measurable, and compatible with the existing architecture.

## Before Opening A Pull Request

- Work from a topic branch or fork; do not ask for direct write access to `main`.
- Run `cargo fmt --check`.
- Run `cargo test`.
- Run `cargo check -p mge-cli --bins` when CLI, MCP, TUI, release, or SDK-facing code changes.
- Run `scripts/smoke-release.ps1` on Windows or `scripts/smoke-release.sh` on Linux/macOS when release packaging changes.
- Do not commit binaries, generated stores, target directories, passphrases, `.env` files, or private corpus data.

## Preview Feedback Checklist

When reporting preview feedback, include:

- OS and terminal used.
- Install path used: release archive, source checkout, or local install script.
- First command that failed or felt unclear.
- Surface used: CLI, TUI, MCP/JSON-RPC, Python SDK, or TypeScript SDK.
- Whether encrypted mode was used.
- What you expected agent memory to remember or recall.

Do not attach private Memory Genome stores, real secrets, passphrases, private corpus data, or generated logs containing sensitive content.

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
- package-specific notes in the SDK-local README files.

Before adding a Markdown file, prefer extending the existing canonical document when the topic already has a clear home. Keep translated entry pages synchronized in meaning without duplicating long reference sections.

## Pull Request Style

- Explain the user-visible behavior change.
- List tests and smokes run.
- Call out any skipped platform checks.
- Keep PRs small enough to review. Split unrelated core, UI, docs, and release changes.
- Keep generated benchmark output out of the repository unless it is a small intentional fixture.
