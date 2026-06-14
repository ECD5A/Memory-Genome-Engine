# Benchmarks

[Russian version](BENCHMARKS.ru.md)

Memory Genome Engine benchmarks are developer/debug tools. They emit JSON reports so results can be diffed and archived, but JSON is not runtime storage. Runtime storage remains binary: `manifest.mgm`, `dictionary/markers.mgd`, `hot/hot.mgl`, `pages/*.mgp`, and `indexes/*.mgi`.

## Synthetic Bench

Use `mge-synthetic-bench` when you need repeatable exact-vs-BinaryFuse checks on generated memory cells:

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

This benchmark is useful for smoke testing candidate page index correctness. It checks that focused exact candidates are a subset of BinaryFuse candidates.

## Corpus Bench

Use `mge-corpus-bench` for local real-workload measurement:

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

The tool reads local text/code files only, skips unsupported binary extensions, skips symlinks, never executes corpus files, never installs dependencies, does not modify the corpus, and writes generated stores only under `--store-root`.

Generated profiles are available when no external corpus is ready:

```bash
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile small --store-root ../mge-bench-small --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile medium --store-root ../mge-bench-medium --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile code-heavy --store-root ../mge-bench-code --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile docs-heavy --store-root ../mge-bench-docs --seed 1
cargo run -p mge-cli --bin mge-corpus-bench -- --generated --profile mixed --store-root ../mge-bench-mixed --seed 1
```

## Reading The Report

`hot` recall measures recall before sealing, from L1 Hot RAM plus pending durable hot persistence.

`sealed cold` recall opens the store for each query. It includes open/recovery, page read/decode, filtering, ranking, and ContextPacket construction.

`sealed repeated` recall reuses the same engine instance. This shows decoded page cache and runtime scoring cache locality.

`ExactMarkerPageIndex` is the default reliable baseline. `BinaryFusePageIndex` is optional and probabilistic: it may return extra candidate pages, but it must not miss exact candidates when filters are built correctly.

`locality benefit` estimates how much faster repeated sealed recall is than cold sealed recall. A high value means cache reuse is helping.

`page decode share` estimates how much repeated focused recall time is spent decoding loaded sealed pages. If this dominates consistently on large real corpora, a future page codec design discussion may be justified.

`scoring/filtering share` is an inclusive estimate for the cell filtering and scoring part of repeated focused recall. It may include nested work already accounted by detailed timing fields, so use it as a bottleneck signal, not as an exact accounting sum.

`ContextPacket share` estimates the cost of building returned memory items and debug details.

## Codec Decision Rule

Do not start a custom page codec just because MessagePack appears in the stack. A custom codec is not justified when:

- page decode share is small or moderate;
- scoring/filtering dominates repeated recall;
- cold recall is dominated by open/read path rather than decode;
- the corpus is generated or too small.

A custom codec may be justified later only if a large real corpus shows page decode dominating repeated sealed recall and simpler cache/policy changes do not address the bottleneck.

## Mandate 1 Baseline

Current generated and repo-local benchmark runs show:

- L1 Hot RAM is not the bottleneck.
- Sealed repeated recall is stable enough for developer-ready core work.
- BinaryFuse is useful as an optional backend, but not consistently enough to replace exact.
- Page decode does not currently justify custom codec work.
- Scoring/filtering should only be revisited after a larger user-provided corpus confirms the same bottleneck.
