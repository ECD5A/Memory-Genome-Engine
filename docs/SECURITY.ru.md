# Security Model

[English version](SECURITY.md)

Мандат 3 добавляет security/encryption layer для Memory Genome Engine. Этот документ фиксирует design boundary перед реализацией. Он специально честно разделяет: что защищаем, что планируем, и что пока не защищаем.

Текущее состояние:

- Текущие stores ещё не зашифрованы.
- `NoSecurity` - честная pass-through реализация.
- Проект не должен заявлять encryption, пока authenticated encryption реально не реализована и не покрыта тестами.
- JSON остаётся только protocol/debug/benchmark output, а не runtime storage.

## Goals

Memory Genome Engine должен стать local-first memory engine, где store может быть locked at rest и unlocked для runtime session.

Основные цели:

- защищать memory payloads в local store files;
- не допускать silent fallback из encrypted mode в plaintext;
- не класть passphrase в manifest, logs, debug output, shell history и SDK errors;
- сохранить binary headers, versioning и validation usable;
- сохранить работу существующих unencrypted stores;
- сохранить текущий memory flow:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Protected Assets

Мандат 3 защищает payload bytes:

- `hot/hot.mgl`
- `hot/snapshot.mgs`
- `pages/*.mgp`
- optional exports/backups позже, только если это явно спроектировано

Главная ценность - `MemoryCell` content и связанные cell payload data, которые раскрывают пользовательскую память при копировании или просмотре store directory.

## Threats In Scope

Первый encrypted mode должен защищать от:

- casual local file inspection;
- украденного или скопированного `.memory-genome` store directory;
- accidental leakage через store files;
- accidental leakage через unencrypted hot logs или snapshots после завершения процесса.

## Threats Out Of Scope

Первый encrypted mode не защищает от:

- compromised running process;
- malicious OS, root, administrator, kernel или debugger access;
- model/API provider, если host явно отправил данные в модель;
- plaintext `ContextPacket` в process memory;
- plaintext Markdown export, если пользователь явно сделал export;
- clipboard, terminal history, shell history или screen capture вне engine;
- denial of service, file deletion, ransomware или rollback attacks без отдельного authenticated backup/audit design.

## Metadata Policy

Security балансируется с deterministic recall и validation. В Мандате 3 сначала шифруем sensitive payloads и явно документируем plaintext metadata.

Metadata, которая может остаться plaintext на первом этапе:

- binary file magic, file kind, format version, codec id, payload length и integrity fields;
- manifest security mode и KDF/AEAD parameters, которые не являются секретом;
- page ids, file names, encoded sizes, codec/compression identifiers;
- page catalog summaries для pruning: marker summaries, scope/kind/status/sensitivity/trust summaries;
- candidate page indexes: exact marker index и Binary Fuse page filters;
- marker dictionary entries, пока не реализованы blind marker tokens.

Payloads, которые должны быть encrypted:

- hot record payloads в `hot/hot.mgl`;
- hot snapshot payloads в `hot/snapshot.mgs`;
- sealed page payload bytes в `pages/*.mgp`;
- future export/backup payloads только если это явно запрошено и задокументировано.

Риски plaintext metadata:

- marker strings, scope names, kinds, statuses, sensitivity labels и page summaries могут раскрывать структуру или темы даже при encrypted content;
- exact marker indexes раскрывают marker-to-page relationships;
- Binary Fuse filters могут раскрывать approximate membership behavior;
- file sizes и page counts раскрывают workload shape.

Позже можно добавить blind marker tokens, encrypted indexes, encrypted/redacted catalogs и policy-gated export. Это отдельные design steps и должны быть оправданы перед реализацией.

## Session Model

Encrypted stores locked at rest.

Ожидаемый session flow:

```text
open encrypted store -> locked-store error
unlock with passphrase/env/key source -> derive session key -> load hot memory -> recall/remember/seal -> process exits -> key gone
```

Правила:

- passphrases никогда не хранятся в manifest;
- session keys могут существовать в RAM, пока процесс активен;
- L1 Hot RAM может держать plaintext cells, пока store unlocked;
- decoded sealed page cache и scoring cache могут держать plaintext-derived data, пока store unlocked;
- после завершения процесса key исчезает и store снова требует unlock;
- lock/unlock process-local, пока не будет явно спроектирован future session service.

## Encryption Design

Не писать собственную криптографию.

Предпочтительное направление реализации:

- AEAD: `chacha20poly1305` with XChaCha20-Poly1305 if available, иначе ChaCha20-Poly1305 со строгими unique nonces;
- KDF: `argon2` для passphrase-to-key derivation;
- random salt/nonce generation: `rand` или `rand_core` через стабильные Rust crypto ecosystem APIs;
- memory hygiene helpers: `zeroize` или `secrecy`, где practical.

Encrypted payload envelope должен содержать non-secret metadata для decryption:

- encryption scheme/version;
- KDF id и parameters;
- salt id или salt bytes where appropriate;
- nonce;
- AEAD tag как часть ciphertext/envelope;
- optional associated-data context.

Associated data должно по возможности связывать ciphertext с контекстом:

- file kind;
- format version;
- page id или frame kind;
- store id, если он будет добавлен позже.

Binary file headers остаются readable. Payload за этими headers encrypted. Existing payload checksum полезен для corruption detection stored payload bytes, но AEAD authentication является главным integrity check для plaintext recovery.

## Storage Layout Direction

Не переписывать всю storage architecture.

Разрешённое направление:

- сохранить текущие file names и binary frame headers;
- добавить security metadata в manifest или clearly versioned security metadata block;
- использовать encrypted payload bytes внутри существующих `.mgl`, `.mgs` и `.mgp` containers;
- сохранить чтение unencrypted stores;
- reject encrypted stores без key;
- wrong keys должны падать ясно.

Migration story:

- existing unencrypted stores продолжают открываться как unencrypted;
- new encrypted stores могут создаваться через `mge init --encrypted`;
- converting existing unencrypted store to encrypted должен быть отдельной явной командой позже, а не silent config flip;
- encrypted stores никогда не должны silently downgrade/fallback to plaintext.

## CLI And Integration Behavior

Предпочтительное local key handling:

- environment variable для automation, например `MGE_PASSPHRASE`;
- prompt input для interactive CLI, где safe;
- key file только если явно задокументировано;
- test-only passphrase flags только в tests/test fixtures.

Избегать:

- passphrase как обычный CLI positional argument;
- logging passphrases;
- passphrase в MCP/SDK errors;
- passphrase в config, manifest, benchmark output или debug output.

Ожидаемая future CLI форма:

```bash
mge init --encrypted
mge unlock --store .memory-genome
mge config security
MGE_PASSPHRASE=... mge remember "..."
```

MCP/SDK behavior:

- locked encrypted stores возвращают structured locked-store error;
- wrong keys возвращают structured authentication/unlock error;
- SDK передают safe security options без дублирования crypto;
- MCP/SDK protocol output не должен содержать passphrases или raw key material.

## Validation And Rebuild

Validation должна работать в двух уровнях:

- locked encrypted store: проверять readable headers, manifest security metadata и file presence where possible;
- unlocked encrypted store: decrypt/authenticate payloads, validate checksums, page catalog, indexes, hot log recovery и rebuild behavior.

`rebuild-indexes` для encrypted stores требует unlocked session, потому что rebuild читает page contents, если только заранее не сохранено достаточно safe plaintext metadata.

## Implementation Gates

Перед implementation:

- зафиксировать design и limitations;
- выбрать dependencies;
- добавить tests, доказывающие отсутствие silent plaintext fallback.

Implementation должна добавить tests:

- unencrypted stores still work;
- encrypted init works;
- encrypted remember/recall/seal works after unlock;
- encrypted store cannot open without key;
- wrong key fails clearly;
- `hot/hot.mgl` payload is not plaintext;
- `hot/snapshot.mgs` payload is not plaintext;
- sealed page payload is not plaintext;
- checkpoint/recovery works encrypted;
- corrupted final encrypted frame handling preserves earlier valid data;
- validate/deep validate behavior is clear;
- MCP locked-store error shape is stable;
- SDK encrypted smoke if safe;
- JSON remains protocol/debug only, not runtime storage.

## Current Limitations

- Encryption designed, but not implemented yet.
- Metadata and indexes are not blind.
- Markdown export plaintext by design, пока не добавлен future encrypted export mode.
- Runtime process memory может содержать plaintext while unlocked.
- Текущий `NoSecurity` не является encryption.
