# Security Model

[Russian version](SECURITY.ru.md)

Mandate 3 adds the security and encryption layer for Memory Genome Engine. This document is the design boundary before implementation. It is intentionally explicit about what is protected now, what is planned, and what is not protected.

Current implementation status:

- Existing unencrypted stores continue to work unchanged.
- `mge init --encrypted` can create an encrypted-mode store marker in `manifest.mgm`.
- Encrypted-mode stores intentionally return a locked-store error for payload operations until session unlock and authenticated encryption are implemented.
- `mge config security` can read manifest-level security status without opening payloads.
- Payload bytes are not encrypted yet.
- `NoSecurity` is an honest pass-through implementation.
- This project must not claim encryption until authenticated encryption is actually implemented and tested.
- JSON remains protocol/debug/benchmark output only, not runtime storage.

## Goals

Memory Genome Engine should become a local-first memory engine where the store can be locked at rest and unlocked for a runtime session.

Primary security goals:

- protect memory payloads in local store files;
- avoid silent fallback from encrypted mode to plaintext;
- keep passphrases out of manifests, logs, debug output, shell history, and SDK errors;
- keep binary headers, versioning, and validation usable;
- keep existing unencrypted stores working;
- keep the existing memory flow unchanged:

```text
remember -> L1 Hot RAM + pending persistence -> hot/hot.mgl
seal -> Sealed Pages + Indexes
recall -> ContextPacket
```

## Protected Assets

Mandate 3 protects the payload bytes of:

- `hot/hot.mgl`
- `hot/snapshot.mgs`
- `pages/*.mgp`
- optional exports/backups later, only when explicitly designed

The high-value data is the `MemoryCell` content and associated cell payload data that would reveal user memory if the store directory is copied or inspected.

## Threats In Scope

The first encrypted mode is intended to protect against:

- casual local file inspection;
- a stolen or copied `.memory-genome` store directory;
- accidental leakage through store files;
- accidental leakage through unencrypted hot logs or snapshots after process exit.

## Threats Out Of Scope

The first encrypted mode does not protect against:

- a compromised running process;
- malicious OS, root, administrator, kernel, or debugger access;
- a model/API provider seeing data that the host explicitly sends to that model;
- plaintext `ContextPacket` data while it is in process memory;
- plaintext Markdown export when the user explicitly exports memory;
- clipboard, terminal history, shell history, or screen capture outside the engine;
- denial of service, file deletion, ransomware, or rollback attacks without a separate authenticated backup/audit design.

## Metadata Policy

Security is balanced against deterministic recall and validation. Mandate 3 should encrypt sensitive payloads first and document any plaintext metadata.

Metadata that may remain plaintext initially:

- binary file magic, file kind, format version, codec id, payload length, and integrity fields;
- manifest security mode and KDF/AEAD parameters that are not secret;
- page ids, file names, encoded sizes, codec/compression identifiers;
- page catalog summaries needed for pruning: marker summaries, scope/kind/status/sensitivity/trust summaries;
- candidate page indexes: exact marker index and Binary Fuse page filters;
- marker dictionary entries unless blind marker tokens are implemented later.

Payloads that should be encrypted:

- hot record payloads in `hot/hot.mgl`;
- hot snapshot payloads in `hot/snapshot.mgs`;
- sealed page payload bytes in `pages/*.mgp`;
- future export/backup payloads only if explicitly requested and documented.

Risks from plaintext metadata:

- marker strings, scope names, kinds, statuses, sensitivity labels, and page-level summaries may reveal structure or topics even when cell content is encrypted;
- exact marker indexes can reveal marker-to-page relationships;
- Binary Fuse filters can reveal approximate membership behavior by design;
- file sizes and page counts can reveal workload shape.

Later hardening can add blind marker tokens, encrypted indexes, encrypted or redacted catalogs, and policy-gated export. Those are future design steps and must be benchmarked or justified before implementation.

## Session Model

Encrypted stores are locked at rest.

Expected session flow:

```text
open encrypted store -> locked-store error
unlock with passphrase/env/key source -> derive session key -> load hot memory -> recall/remember/seal -> process exits -> key gone
```

Rules:

- passphrases are never stored in the manifest;
- session keys may exist in RAM while the process is active;
- L1 Hot RAM may hold plaintext cells while unlocked;
- decoded sealed page cache and scoring cache may hold plaintext-derived data while unlocked;
- after process exit, the key is gone and the store requires unlock again;
- lock/unlock is process-local unless a future session service is explicitly designed.

## Encryption Design

Do not write custom cryptography.

Preferred implementation direction:

- AEAD: `chacha20poly1305` with XChaCha20-Poly1305 if available, otherwise ChaCha20-Poly1305 with strict unique nonces;
- KDF: `argon2` for passphrase-to-key derivation;
- random salt/nonce generation: `rand` or `rand_core` through stable Rust crypto ecosystem APIs;
- memory hygiene helpers: `zeroize` or `secrecy` where practical.

The encrypted payload envelope should include non-secret metadata needed for decryption:

- encryption scheme/version;
- KDF id and parameters;
- salt id or salt bytes where appropriate;
- nonce;
- AEAD tag as part of ciphertext or envelope;
- optional associated-data context.

Associated data should bind ciphertext to store/file context where practical:

- file kind;
- format version;
- page id or frame kind where applicable;
- store id if added later.

Binary file headers remain readable. The payload behind those headers is encrypted. The existing payload checksum remains useful for corruption detection of the stored payload bytes, but AEAD authentication is the real integrity check for plaintext recovery.

## Storage Layout Direction

Do not rewrite the whole storage architecture.

Allowed direction:

- keep current file names and binary frame headers;
- add security metadata to manifest or a clearly versioned security metadata block;
- use encrypted payload bytes inside existing `.mgl`, `.mgs`, and `.mgp` containers;
- keep unencrypted stores readable;
- reject encrypted stores without a key;
- fail clearly on wrong keys.

Migration story:

- existing unencrypted stores continue to open as unencrypted;
- new encrypted stores may be initialized with `mge init --encrypted`;
- converting an existing unencrypted store to encrypted should be an explicit later command, not a silent config flip;
- encrypted stores must never silently downgrade or fallback to plaintext.

## CLI And Integration Behavior

Preferred local key handling:

- environment variable for automation, for example `MGE_PASSPHRASE`;
- prompt input for interactive CLI when safe;
- key file only if explicitly documented;
- test-only passphrase flags only inside tests or test fixtures.

Avoid:

- passphrase as a normal CLI positional argument;
- logging passphrases;
- returning passphrases through MCP/SDK errors;
- storing passphrases in config, manifest, benchmark output, or debug output.

Current and expected CLI shape:

```bash
mge init --encrypted
mge config security
# Future, not implemented yet:
mge unlock --store .memory-genome
MGE_PASSPHRASE=... mge remember "..."
```

MCP/SDK behavior:

- locked encrypted stores return a structured locked-store error (`details.error_kind = "store_locked"` in the JSON-RPC adapter);
- wrong keys return a structured authentication/unlock error;
- SDKs pass safe security options through without duplicating crypto;
- MCP/SDK protocol output must not include passphrases or raw key material.

## Validation And Rebuild

Validation should work in two levels:

- locked encrypted store: verify readable headers, manifest security metadata, and file presence where possible;
- unlocked encrypted store: decrypt/authenticate payloads, validate checksums, page catalog, indexes, hot log recovery, and rebuild behavior.

`rebuild-indexes` on encrypted stores needs an unlocked session because rebuilding requires reading page contents unless enough safe plaintext metadata is deliberately retained.

## Implementation Gates

Before implementation:

- document the design and limitations;
- choose dependencies;
- add tests that prove no silent plaintext fallback.

Implementation must add tests for:

- unencrypted stores still work;
- encrypted init works;
- encrypted remember/recall/seal works after unlock;
- encrypted store cannot open without key;
- wrong key fails clearly;
- `hot/hot.mgl` payload is not plaintext;
- `hot/snapshot.mgs` payload is not plaintext;
- sealed page payload is not plaintext;
- checkpoint/recovery works encrypted;
- corrupted final encrypted frame handling still preserves earlier valid data;
- validate/deep validate behavior is clear;
- MCP locked-store error shape is stable;
- SDK encrypted smoke if safe;
- JSON remains protocol/debug only, not runtime storage.

## Current Limitations

- Encryption is designed and the manifest-level encrypted/locked foundation is implemented, but authenticated payload encryption is not yet implemented.
- There is no session unlock command yet.
- Stores created with `mge init --encrypted` are locked for remember/recall/seal/checkpoint/stats/validate/rebuild/export until unlock/encryption support lands.
- Metadata and indexes are not blind.
- Markdown export is plaintext by design unless a future encrypted export mode is added.
- Runtime process memory can contain plaintext while unlocked.
- The current `NoSecurity` implementation is not encryption.
