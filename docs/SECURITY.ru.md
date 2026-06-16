# Модель Безопасности

[English version](SECURITY.md)

Этот документ описывает security/encryption model для Memory Genome Engine. Текущий реализованный объём: session unlock и authenticated encryption для L1 hot storage и sealed page payloads.

Текущий статус реализации:

- Существующие unencrypted stores продолжают работать без изменений.
- `mge init --encrypted` создаёт encrypted-mode store.
- `mge init --encrypted --passphrase-env MGE_PASSPHRASE` создаёт metadata для проверки ключа и открывает первую session.
- Encrypted stores требуют session unlock для payload operations через `--passphrase-env`.
- Hot records в `hot/hot.mgl` шифруются и аутентифицируются, если store создан с key metadata.
- Checkpoint payload в `hot/snapshot.mgs` шифруется и аутентифицируется, если store создан с key metadata.
- Sealed page payloads в `pages/*.mgp` шифруются и аутентифицируются, если store создан с key metadata.
- Неверный passphrase даёт понятную authentication error.
- Encrypted init без passphrase всё ещё разрешён как locked marker/config state, но payload operations остаются locked, пока нет key metadata.
- Encrypted-mode payload operations не делают silent fallback в plaintext; они падают как locked/authenticated failure.
- `mge config security` читает безопасный manifest-level security status без открытия payload.
- Indexes, marker dictionary, page catalog summaries, Markdown export и часть manifest metadata в этом проходе не шифруются.
- JSON остаётся только protocol/debug/benchmark output, а не runtime storage.

## Core Flow

Архитектура памяти не менялась:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

Encryption package меняет только payload persistence и session unlock. Он не меняет recall modes, `MarkerGenome`, `MemoryCell.markers`, candidate indexes, page codec или storage layout.

## Crypto Dependencies

Используются известные Rust crypto crates; custom crypto не пишется:

- `chacha20poly1305`: XChaCha20-Poly1305 AEAD для authenticated payload encryption.
- `argon2`: Argon2id passphrase-to-session-key derivation.
- `rand`: OS randomness для KDF salt и AEAD nonce.
- `zeroize`: zeroize runtime `SessionKey` bytes при drop.

Passphrase никогда не хранится в `manifest.mgm`, logs, debug output или errors. Derived key живёт только в runtime session процесса.

## Session Unlock

Рекомендуемый CLI flow:

```powershell
$env:MGE_PASSPHRASE = "use a real passphrase outside shell history"
mge init --encrypted --passphrase-env MGE_PASSPHRASE
mge remember "private hot memory" --passphrase-env MGE_PASSPHRASE
mge recall "private hot memory" --passphrase-env MGE_PASSPHRASE
mge checkpoint --passphrase-env MGE_PASSPHRASE
mge seal --passphrase-env MGE_PASSPHRASE
mge validate --passphrase-env MGE_PASSPHRASE
```

`--passphrase-env` принимает имя переменной окружения, а не сам passphrase.

Encrypted store без unlock возвращает `store is locked`. Неверный passphrase возвращает `authentication failed`.

Ни одна команда не должна downgrade-ить encrypted payload writes в plaintext. Если encrypted store нельзя unlock-нуть, remember, recall, seal, checkpoint, validate, rebuild, stats и export payload operations падают, а не пишут plaintext.

## Что Шифруется Сейчас

Шифруется при init с key metadata:

- hot log record payloads в `hot/hot.mgl`;
- hot checkpoint payloads в `hot/snapshot.mgs`;
- sealed page payloads в `pages/*.mgp`;
- key-check block внутри manifest security metadata.

Пока остаётся plaintext:

- binary frame headers: magic, file kind, version, codec id, payload length, checksum;
- manifest safe metadata, security mode, KDF salt/params, AEAD scheme/version;
- marker dictionary: `dictionary/markers.mgd`;
- indexes and page catalog: `indexes/*.mgi`;
- page catalog summaries: marker/scope/kind/status/sensitivity/trust summaries и encoded-size metadata;
- Markdown export: `exports/memory.md`;
- process memory while the store is unlocked.

Encrypted indexes, blind marker tokens и encrypted export - отдельные будущие пакеты. Index/catalog metadata остаётся plaintext ради deterministic search, pruning, validation и rebuild.

## Encrypted Metadata And Index Risk Model

Текущий encrypted mode сначала защищает payload bytes:

- hot record payloads;
- hot checkpoint snapshots;
- sealed page payloads.

Следующая metadata остаётся plaintext by design:

- manifest safe metadata, включая security mode, storage versions, default codec/compression, index kind и key-derivation params;
- marker dictionary: `dictionary/markers.mgd`;
- page catalog summaries и candidate index files: `indexes/*.mgi`;
- encoded page sizes и binary frame sizes;
- marker, scope, kind, status, sensitivity и trust summaries, которые используются для pruning;
- Markdown export, если пользователь явно его создал.

Risks у plaintext metadata:

- marker names могут раскрывать темы, названия продуктов, людей, repositories или incident areas;
- file, project и scope markers могут раскрывать структуру проекта;
- kind/status/sensitivity/trust summaries могут раскрывать categories вроде decisions, task states, rejected/deprecated memory или наличие secret-reference;
- index size, page count, page sizes и encoded sizes раскрывают примерный объём памяти и паттерны роста;
- access pattern, timing и repeated recall locality не скрываются at-rest encryption;
- Markdown export намеренно human-readable и должен считаться plaintext disclosure.

Почему это сейчас остаётся plaintext:

- recall зависит от marker/page pruning до page payload decode;
- `ExactMarkerPageIndex` и `BinaryFusePageIndex` требуют deterministic marker IDs, чтобы не уходить в full scan;
- `validate --deep` и `rebuild-indexes` требуют проверки catalog/index consistency по existing pages;
- performance резко просядет, если encrypted stores будут decode-ить каждую page на каждый query;
- существующие stores и tools завязаны на текущие dictionary/index/catalog boundaries;
- metadata privacy нужно менять отдельным index design, а не скрытым storage tweak.

### Metadata Privacy Design Options

Option A: current mode, encrypted payloads with plaintext metadata/indexes.

- Private: hot content, checkpoint content, sealed page MemoryCell payloads.
- Still leaked: marker strings, scope/kind/status/sensitivity/trust summaries, index shape, page counts/sizes.
- Performance: лучший текущий путь; exact и Binary Fuse pruning остаются быстрыми.
- Validate/rebuild: без изменений и надёжно.
- Compatibility: без API или storage-layout break.
- Migration complexity: none.
- `CandidatePageIndex` API: без изменений.
- Binary Fuse: остаётся useful как optional static page filter.

Option B: hashed marker dictionary, canonical marker string -> keyed hash / opaque ID.

- Private: raw marker strings больше не хранятся напрямую в dictionary, если key доступен.
- Still leaked: equality, frequency, page/index shape, marker reuse patterns и любые unblinded manifest/catalog fields.
- Performance: близко к current, если hashes deterministic после unlock.
- Validate/rebuild: должны работать после unlock и проверять keyed hashes вместо strings.
- Compatibility: старые plaintext dictionaries требуют migration или dual-read support.
- Migration complexity: medium.
- `CandidatePageIndex` API: скорее всего можно сохранить, если `MarkerId` остаётся public index unit.
- Binary Fuse: остаётся useful, если filters строятся по stable keyed IDs/fingerprints.

Option C: blind marker indexes using keyed marker tokens for catalog and indexes.

- Private: catalog/index files больше не раскрывают raw marker names и снижают topic leakage.
- Still leaked: token equality, token frequency, page counts, page sizes, status of page hits, timing и access patterns.
- Performance: должен быть близок к current, если query markers один раз превращаются в keyed tokens после unlock.
- Validate/rebuild: требует unlock; rebuild должен regenerate keyed tokens из decrypted pages или keyed dictionary.
- Compatibility: existing indexes требуют rebuild; для old plaintext catalog/index files нужна explicit migration policy.
- Migration complexity: medium to high.
- `CandidatePageIndex` API: должен остаться unchanged, если tokens укладываются в существующий `MarkerId`/u64-style path; не добавлять новую filter family.
- Binary Fuse: остаётся useful, потому что Binary Fuse может работать по keyed marker fingerprints вместо raw marker IDs.

Option D: encrypted dictionary with plaintext derived index IDs.

- Private: raw marker strings at rest в dictionary.
- Still leaked: derived ID equality/frequency, index membership, catalog summaries, page sizes и stable ID correlations across backups.
- Performance: почти current после unlock, потому что indexes всё ещё используют plaintext derived IDs.
- Validate/rebuild: dictionary operations требуют unlock; indexes можно проверять structurally.
- Compatibility: moderate; меняется dictionary read/write path, indexes в основном остаются стабильными.
- Migration complexity: medium.
- `CandidatePageIndex` API: unchanged.
- Binary Fuse: остаётся useful.

Option E: fully encrypted metadata.

- Private: максимальная at-rest metadata privacy для dictionary, catalog, summaries и indexes.
- Still leaked: file count, file sizes, timestamps, access timing и process-memory data while unlocked.
- Performance: вероятно намного медленнее без нового private index design; naive mode превращается в page full-scan или broad decrypt/decode.
- Validate/rebuild: требует unlock и может стать expensive; offline structural validation будет ограничен.
- Compatibility: высокий break risk для tools, smokes и mixed stores.
- Migration complexity: high.
- `CandidatePageIndex` API: вероятно требует redesign или adapter layer.
- Binary Fuse: useful только если rebuilt по keyed/private tokens; иначе теряет текущую роль.

Recommendation for Memory Genome Engine:

- Оставить current payload-encrypted mode как default encrypted mode.
- Честно документировать plaintext metadata leakage и считать это explicit tradeoff, а не скрытым bug.
- Добавлять optional blind marker mode позже только после prototype, который докажет recall correctness, rebuild behavior и benchmark impact.
- Не шифровать всю metadata по умолчанию; это подтолкнёт recall к медленным full scans и усложнит validation.
- Не ломать `CandidatePageIndex`, `MarkerGenome`, recall modes или Exact/BinaryFuse strategy.
- Не создавать filter zoo. Если blind mode будет добавлен, reuse existing exact/BinaryFuse boundary через keyed marker IDs или keyed marker fingerprints.

Future phased plan, не реализуется сейчас:

- Phase 1: keyed marker fingerprints для encrypted stores, чтобы снизить plaintext dictionary risk, сохранив public marker/query input и existing index API.
- Phase 2: catalog summaries и index files переходят на keyed marker IDs/fingerprints; validate/rebuild требуют unlock и explicit rebuild existing indexes.
- Phase 3: optional higher-privacy mode с reduced pruning или более дорогой validation для users, которые принимают slower recall.

## Recovery

Hot recovery остаётся crash-safe:

- после unlock `hot/snapshot.mgs` восстанавливает checkpointed hot cells;
- `hot/hot.mgl` replay-ит valid encrypted hot records после snapshot offset;
- sealed recall decrypt-ит payloads из `pages/*.mgp` после session unlock;
- corrupted/truncated final encrypted frame отбрасывается без уничтожения предыдущей valid hot memory;
- wrong key падает на authentication до нормального payload use.

## MCP И SDK

MCP/JSON-RPC tools принимают optional `passphrase_env` в params. Adapter никогда не принимает и не возвращает raw passphrase.

Error mapping:

- locked encrypted store без unlock: `details.error_kind = "store_locked"`;
- wrong passphrase/authentication failure: `details.error_kind = "auth_failed"`;
- invalid env var name или empty env value: structured command/parameter error на caller path.

Python SDK использует `passphrase_env="MGE_PASSPHRASE"`. TypeScript SDK использует `passphraseEnv: "MGE_PASSPHRASE"`. Оба wrapper-а делегируют crypto и storage logic в Rust CLI/core.

Encrypted sealed recall, `validate --deep` и `rebuild-indexes` требуют тот же unlock path. Missing unlock мапится в `store_locked`; wrong passphrase и AEAD authentication failure мапятся в `auth_failed`.

## Threats In Scope

Этот pass защищает от casual local inspection hot memory files и sealed page payloads, если у атакующего нет passphrase.

## Threats Out Of Scope

Этот pass не защищает от:

- compromised running process;
- malicious OS/root/admin/debugger access;
- plaintext `ContextPacket` в process memory;
- plaintext marker dictionary и index/catalog metadata;
- plaintext Markdown export;
- shell history, terminal capture, clipboard или host-side logging;
- file deletion, rollback attacks или ransomware.

## Текущие Ограничения

- Hot storage и sealed page payloads шифруются для encrypted stores с key metadata.
- Marker dictionary и candidate index metadata ещё не blind.
- Interactive prompt unlock command пока нет; безопасный CLI unlock сейчас через `--passphrase-env`.
- Encrypted export mode пока нет.
- Existing unencrypted stores не мигрируются в encrypted автоматически.

## Future Security Work

Payload encryption готов для текущего продукта. Future security work является non-blocking и должен начинаться только с отдельного design:

- optional keyed marker fingerprints / blind marker metadata prototype;
- encrypted indexes или encrypted marker dictionary только после benchmark и migration evidence;
- encrypted Markdown export mode;
- interactive unlock prompt или host key-management integration.

Не шифровать indexes, marker dictionary или catalog summaries по умолчанию, потому что эти структуры определяют recall pruning и validation behavior.
