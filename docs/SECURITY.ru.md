# Модель Безопасности

[English version](SECURITY.md)

Мандат 3 добавляет security/encryption слой для Memory Genome Engine. Текущий реализованный объём: session unlock и authenticated encryption для L1 hot storage.

Текущий статус реализации:

- Существующие unencrypted stores продолжают работать без изменений.
- `mge init --encrypted` создаёт encrypted-mode store.
- `mge init --encrypted --passphrase-env MGE_PASSPHRASE` создаёт metadata для проверки ключа и открывает первую session.
- Encrypted stores требуют session unlock для payload operations через `--passphrase-env`.
- Hot records в `hot/hot.mgl` шифруются и аутентифицируются, если store создан с key metadata.
- Checkpoint payload в `hot/snapshot.mgs` шифруется и аутентифицируется, если store создан с key metadata.
- Неверный passphrase даёт понятную authentication error.
- Encrypted init без passphrase всё ещё разрешён как locked marker/config state, но payload operations остаются locked, пока нет key metadata.
- `mge config security` читает безопасный manifest-level security status без открытия payload.
- Sealed pages, indexes, marker dictionary, page catalog summaries, Markdown export и часть manifest metadata пока не шифруются.
- JSON остаётся только protocol/debug/benchmark output, а не runtime storage.

## Core Flow

Архитектура памяти не менялась:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

Encryption package меняет только hot payload persistence и session unlock. Он не меняет recall modes, `MarkerGenome`, `MemoryCell.markers`, candidate indexes, sealed page codec или storage layout.

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
mge validate --passphrase-env MGE_PASSPHRASE
```

`--passphrase-env` принимает имя переменной окружения, а не сам passphrase.

Encrypted store без unlock возвращает `store is locked`. Неверный passphrase возвращает `authentication failed`.

## Что Шифруется Сейчас

Шифруется при init с key metadata:

- hot log record payloads в `hot/hot.mgl`;
- hot checkpoint payloads в `hot/snapshot.mgs`;
- key-check block внутри manifest security metadata.

Пока остаётся plaintext:

- binary frame headers: magic, file kind, version, codec id, payload length, checksum;
- manifest safe metadata, security mode, KDF salt/params, AEAD scheme/version;
- marker dictionary: `dictionary/markers.mgd`;
- sealed pages: `pages/*.mgp`;
- indexes and page catalog: `indexes/*.mgi`;
- Markdown export: `exports/memory.md`;
- process memory while the store is unlocked.

Sealed page encryption, encrypted indexes, blind marker tokens и encrypted export - отдельные будущие пакеты.

## Recovery

Hot recovery остаётся crash-safe:

- после unlock `hot/snapshot.mgs` восстанавливает checkpointed hot cells;
- `hot/hot.mgl` replay-ит valid encrypted hot records после snapshot offset;
- corrupted/truncated final encrypted frame отбрасывается без уничтожения предыдущей valid hot memory;
- wrong key падает на authentication до нормального payload use.

## MCP И SDK

MCP/JSON-RPC tools принимают optional `passphrase_env` в params. Adapter никогда не принимает и не возвращает raw passphrase.

Error mapping:

- locked encrypted store без unlock: `details.error_kind = "store_locked"`;
- wrong passphrase/authentication failure: `details.error_kind = "auth_failed"`;
- invalid env var name или empty env value: structured command/parameter error на caller path.

Python SDK использует `passphrase_env="MGE_PASSPHRASE"`. TypeScript SDK использует `passphraseEnv: "MGE_PASSPHRASE"`. Оба wrapper-а делегируют crypto и storage logic в Rust CLI/core.

## Threats In Scope

Этот pass защищает от casual local inspection hot memory files и скопированных hot logs/snapshots, если у атакующего нет passphrase.

## Threats Out Of Scope

Этот pass не защищает от:

- compromised running process;
- malicious OS/root/admin/debugger access;
- plaintext `ContextPacket` в process memory;
- plaintext sealed pages и index/catalog metadata;
- plaintext Markdown export;
- shell history, terminal capture, clipboard или host-side logging;
- file deletion, rollback attacks или ransomware.

## Текущие Ограничения

- Сейчас шифруются только hot storage payloads.
- Sealed pages ещё не шифруются.
- Marker dictionary и candidate index metadata ещё не blind.
- Interactive prompt unlock command пока нет; безопасный CLI unlock сейчас через `--passphrase-env`.
- Encrypted export mode пока нет.
- Existing unencrypted stores не мигрируются в encrypted автоматически.

## Следующий Security Step

Следующий пакет Мандата 3: sealed page payload encryption. Он должен сохранить явные boundaries для page headers/catalog и не должен молча шифровать indexes или blind marker metadata без отдельного design.
