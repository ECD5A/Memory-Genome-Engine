# Бенчмарки

[English version](BENCHMARKS.md)

Бенчмарки Memory Genome Engine - это developer/debug tools. Они выводят JSON-отчёты, чтобы результаты можно было сравнивать и архивировать, но JSON не является runtime storage. Runtime storage остаётся бинарным: `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp` и `indexes/*.mgi`.

## Synthetic Bench

Используйте `mge-synthetic-bench`, когда нужен повторяемый exact-vs-BinaryFuse smoke на сгенерированных memory cells:

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

Этот benchmark полезен для проверки candidate page index correctness. Он проверяет, что focused exact candidates являются subset для BinaryFuse candidates.

## Corpus Bench

Используйте `mge-corpus-bench` для локального real-workload measurement:

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

Tool читает только локальные text/code files, пропускает unsupported binary extensions, пропускает symlinks, не исполняет corpus files, не устанавливает зависимости, не меняет corpus и пишет generated stores только в `--store-root`.

Generated profiles доступны, если внешнего corpus пока нет:

```bash
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-bench-small --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile medium --store-root ../mge-bench-medium --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile code-heavy --store-root ../mge-bench-code --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile docs-heavy --store-root ../mge-bench-docs --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile mixed --store-root ../mge-bench-mixed --seed 1
```

## Как Читать Отчёт

`hot` recall измеряет recall до seal, из L1 Hot RAM плюс pending durable hot persistence.

`sealed cold` recall открывает store для каждого query. Здесь учитываются open/recovery, page read/decode, filtering, ranking и ContextPacket construction.

`sealed repeated` recall переиспользует один engine instance. Это показывает decoded page cache и runtime scoring cache locality.

`ExactMarkerPageIndex` - default reliable baseline. `BinaryFusePageIndex` - optional probabilistic backend: он может вернуть лишние candidate pages, но не должен пропускать exact candidates при корректно построенных filters.

`locality benefit` показывает, насколько repeated sealed recall быстрее cold sealed recall. Высокое значение означает, что cache reuse помогает.

`page decode share` показывает долю repeated focused recall, уходящую на decode загруженных sealed pages. Если это стабильно доминирует на большом real corpus, тогда можно обсуждать future page codec design.

`scoring/filtering share` - inclusive estimate для cell filtering и scoring в repeated focused recall. Он может включать вложенную работу, уже отражённую в detailed timing fields, поэтому это bottleneck signal, а не точная сумма accounting.

`ContextPacket share` показывает стоимость построения returned memory items и debug details.

## Правило По Codec

Не надо начинать custom page codec только потому, что в stack есть MessagePack. Custom codec не обоснован, когда:

- page decode share small или moderate;
- scoring/filtering доминирует repeated recall;
- cold recall больше зависит от open/read path, чем от decode;
- corpus generated или слишком маленький.

Custom codec может быть обоснован позже только если большой real corpus покажет, что page decode стабильно доминирует repeated sealed recall, а более простые cache/policy changes не решают проблему.

## Mandate 1 Baseline

Текущие generated и repo-local benchmark runs показывают:

- L1 Hot RAM не является bottleneck.
- Sealed repeated recall достаточно стабилен для developer-ready core.
- BinaryFuse полезен как optional backend, но не настолько стабилен, чтобы заменить exact.
- Page decode сейчас не обосновывает custom codec work.
- Scoring/filtering стоит трогать только после большого user-provided corpus, если он подтвердит тот же bottleneck.
