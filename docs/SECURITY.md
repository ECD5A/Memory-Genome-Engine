# Security Model

[Russian version](SECURITY.ru.md)

Mandate 3 adds the security and encryption layer for Memory Genome Engine. The current implemented scope is session unlock plus authenticated encryption for L1 hot storage and sealed page payloads.

Current implementation status:

- Existing unencrypted stores continue to work unchanged.
- `mge init --encrypted` creates an encrypted-mode store.
- `mge init --encrypted --passphrase-env MGE_PASSPHRASE` initializes key-verification metadata and unlocks the first session.
- Encrypted stores require session unlock for payload operations through `--passphrase-env`.
- `hot/hot.mgl` hot records are encrypted/authenticated when the store has key metadata.
- `hot/snapshot.mgs` checkpoint payloads are encrypted/authenticated when the store has key metadata.
- `pages/*.mgp` sealed page payloads are encrypted/authenticated when the store has key metadata.
- Wrong passphrases fail with an authentication error.
- Encrypted init without passphrase is still allowed as a locked marker/config state, but payload operations remain locked until key metadata exists.
- `mge config security` reads safe manifest-level security status without opening payloads.
- Indexes, marker dictionary, page catalog summaries, Markdown export, and selected manifest metadata are not encrypted in this pass.
- JSON remains protocol/debug/benchmark output only, not runtime storage.

## Core Flow

The memory architecture is unchanged:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

The encryption work changes only payload persistence and session unlock. It does not change recall modes, `MarkerGenome`, `MemoryCell.markers`, candidate indexes, page codec, or storage layout.

## Crypto Dependencies

The implementation uses well-known Rust crypto crates and does not implement custom crypto:

- `chacha20poly1305`: XChaCha20-Poly1305 AEAD for authenticated payload encryption.
- `argon2`: Argon2id passphrase-to-session-key derivation.
- `rand`: OS randomness for KDF salt and AEAD nonces.
- `zeroize`: zeroizes the runtime `SessionKey` bytes on drop.

The passphrase is never stored in `manifest.mgm`, logs, debug output, or errors. The derived key exists only in the running process session.

## Session Unlock

Recommended CLI flow:

```bash
export MGE_PASSPHRASE="use a real passphrase outside shell history"
mge init --encrypted --passphrase-env MGE_PASSPHRASE
mge remember "private hot memory" --passphrase-env MGE_PASSPHRASE
mge recall "private hot memory" --passphrase-env MGE_PASSPHRASE
mge checkpoint --passphrase-env MGE_PASSPHRASE
mge validate --passphrase-env MGE_PASSPHRASE
```

`--passphrase-env` takes the environment variable name, not the passphrase value.

Encrypted stores without an unlock return `store is locked`. Wrong passphrases return `authentication failed`.

## What Is Encrypted Now

Encrypted when initialized with key metadata:

- hot log record payloads in `hot/hot.mgl`;
- hot checkpoint payloads in `hot/snapshot.mgs`;
- sealed page payloads in `pages/*.mgp`;
- the key-check block inside manifest security metadata.

Still plaintext by design in this pass:

- binary frame headers: magic, file kind, version, codec id, payload length, checksum;
- manifest safe metadata, security mode, KDF salt/parameters, AEAD scheme/version;
- marker dictionary: `dictionary/markers.mgd`;
- indexes and page catalog: `indexes/*.mgi`;
- page catalog summaries: marker/scope/kind/status/sensitivity/trust summaries and encoded-size metadata;
- Markdown export: `exports/memory.md`;
- process memory while the store is unlocked.

Encrypted indexes, blind marker tokens, and encrypted export are separate future packages. Index/catalog metadata remains plaintext for deterministic search, pruning, validation, and rebuild.

## Recovery

Hot recovery remains crash-safe:

- after unlock, `hot/snapshot.mgs` can restore checkpointed hot cells;
- `hot/hot.mgl` replays valid encrypted hot records after the snapshot offset;
- sealed recall decrypts `pages/*.mgp` payloads after session unlock;
- a corrupted or truncated final encrypted frame is discarded without destroying earlier valid hot memory;
- wrong keys fail authentication before normal payload use.

## MCP And SDK

MCP/JSON-RPC tools accept optional `passphrase_env` in params. The adapter never accepts or returns raw passphrases.

Error mapping:

- locked encrypted store without unlock: `details.error_kind = "store_locked"`;
- wrong passphrase/authentication failure: `details.error_kind = "auth_failed"`;
- invalid env var name or empty env value: structured command/parameter error from the caller path.

Python SDK uses `passphrase_env="MGE_PASSPHRASE"`. TypeScript SDK uses `passphraseEnv: "MGE_PASSPHRASE"`. Both wrappers delegate crypto and storage logic to the Rust CLI/core.

Encrypted sealed recall, `validate --deep`, and `rebuild-indexes` require the same unlock path. Missing unlock maps to `store_locked`; wrong passphrases and AEAD authentication failures map to `auth_failed`.

## Threats In Scope

This pass protects against casual local inspection of hot memory files and sealed page payloads when the attacker does not have the passphrase.

## Threats Out Of Scope

This pass does not protect against:

- a compromised running process;
- malicious OS/root/admin/debugger access;
- plaintext `ContextPacket` data in process memory;
- plaintext marker dictionary and index/catalog metadata;
- plaintext Markdown export;
- shell history, terminal capture, clipboard, or host-side logging;
- file deletion, rollback attacks, or ransomware.

## Current Limitations

- Hot storage and sealed page payloads are encrypted for encrypted stores with key metadata.
- Marker dictionary and candidate index metadata are not blind.
- There is no interactive prompt unlock command; current safe CLI unlock uses `--passphrase-env`.
- There is no encrypted export mode.
- Existing unencrypted stores are not migrated to encrypted stores automatically.

## Next Security Step

The next Mandate 3 package should be encrypted indexes / blind marker metadata design. Do not encrypt indexes, marker dictionary, or catalog summaries without a separate design, because those structures define search and pruning behavior.
