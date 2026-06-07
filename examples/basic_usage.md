# Basic Usage

[Русская версия](basic_usage.ru.md)

Run from the repository root:

```bash
cargo run -p mge-cli -- init

cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
```

Expected shape:

```text
Relevant memory:
- User prefers concise technical explanations [kind=user_preference, trust=user_confirmed, status=active, scope=global]

Constraints:
- Do not use deprecated or rejected memories.
- Do not expose secret_reference cells.
```

Seal hot memory into pages:

```bash
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
```

Compact page storage for a new store:

```bash
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

Change defaults for future sealed pages in an existing store:

```bash
cargo run -p mge-cli -- config show
cargo run -p mge-cli -- config set --page-codec messagepack --compression zstd
```
