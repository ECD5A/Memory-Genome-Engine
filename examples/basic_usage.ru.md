# Базовое Использование

[English version](basic_usage.md)

Запускать из корня репозитория:

```bash
cargo run -p mge-cli -- init

cargo run -p mge-cli -- remember "User prefers concise technical explanations" \
  --kind user_preference \
  --scope global \
  --trust user_confirmed

cargo run -p mge-cli -- remember \
  --kind user_preference \
  --subject answer_style \
  --json-value '{"style":"concise","max_examples":2}'

cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
```

Ожидаемая форма:

```text
Relevant memory:
- User prefers concise technical explanations [kind=user_preference, trust=user_confirmed, status=active, scope=global]

Constraints:
- Do not use deprecated or rejected memories.
- Do not expose secret_reference cells.
```

Запечатать hot memory в pages:

```bash
cargo run -p mge-cli -- seal
cargo run -p mge-cli -- recall "How should the agent answer technical questions?"
cargo run -p mge-cli -- stats
cargo run -p mge-cli -- validate
```

Compact page storage для нового хранилища:

```bash
cargo run -p mge-cli -- init --page-codec messagepack --compression zstd
```

Opt-in Binary Fuse candidate page filtering:

```bash
cargo run -p mge-cli -- init --index-kind binary_fuse_page
```

Изменить defaults для будущих sealed pages в существующем хранилище:

```bash
cargo run -p mge-cli -- config show
cargo run -p mge-cli -- config set --page-codec messagepack --compression zstd
cargo run -p mge-cli -- config set --page-clusterer marker_overlap
cargo run -p mge-cli -- config set --index-kind binary_fuse_page
```
