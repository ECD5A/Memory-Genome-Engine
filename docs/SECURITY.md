# Security Model

This document describes the security and encryption model for Memory Genome Engine. The current implemented scope is session unlock plus authenticated encryption for L1 hot storage and sealed page payloads.

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
- Encrypted-mode payload operations do not silently fall back to plaintext; they fail locked/authenticated instead.
- `mge config security` reads safe manifest-level security status without opening payloads.
- Indexes, marker dictionary, page catalog summaries, Markdown export, and selected manifest metadata are not encrypted in this pass.
- JSON remains protocol/debug report output only, not runtime storage.

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
mge seal --passphrase-env MGE_PASSPHRASE
mge validate --passphrase-env MGE_PASSPHRASE
```

`--passphrase-env` takes the environment variable name, not the passphrase value.

Encrypted stores without an unlock return `store is locked`. Wrong passphrases return `authentication failed`.

No command should downgrade encrypted payload writes to plaintext. If an encrypted store cannot be unlocked, remember, recall, seal, checkpoint, validate, rebuild, stats, and export payload operations fail instead.

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

## Encrypted Metadata And Index Risk Model

Current encrypted mode protects payload bytes first:

- hot record payloads;
- hot checkpoint snapshots;
- sealed page payloads.

The following metadata remains plaintext by design:

- manifest safe metadata, including security mode, storage versions, default codec/compression, index kind, and key-derivation parameters;
- marker dictionary: `dictionary/markers.mgd`;
- page catalog summaries and candidate index files: `indexes/*.mgi`;
- encoded page sizes and binary frame sizes;
- marker, scope, kind, status, sensitivity, and trust summaries used for pruning;
- Markdown export if the user explicitly creates it.

Plaintext metadata risks:

- marker names can reveal topics, product names, people, repositories, or incident areas;
- file, project, and scope markers can reveal project structure;
- kind/status/sensitivity/trust summaries can leak categories such as decisions, task states, rejected/deprecated memory, or secret-reference presence;
- index size, page count, page sizes, and encoded sizes leak rough memory volume and growth patterns;
- access pattern, timing, and repeated recall locality are not hidden by at-rest encryption;
- Markdown export is intentionally human-readable and should be treated as plaintext disclosure.

Why this remains plaintext now:

- recall depends on marker/page pruning before page payload decode;
- `ExactMarkerPageIndex` and `BinaryFusePageIndex` need deterministic marker IDs to avoid full scans;
- `validate --deep` and `rebuild-indexes` need catalog/index consistency checks from existing pages;
- performance would regress sharply if encrypted stores had to decode every page for every query;
- existing stores and tools rely on the current dictionary/index/catalog boundaries;
- changing metadata privacy correctly requires a deliberate index design, not a hidden storage tweak.

### Metadata Privacy Design Options

Option A: current mode, encrypted payloads with plaintext metadata/indexes.

- Private: hot content, checkpoint content, sealed page MemoryCell payloads.
- Still leaked: marker strings, scope/kind/status/sensitivity/trust summaries, index shape, page counts/sizes.
- Performance: best current behavior; exact and Binary Fuse pruning remain fast.
- Validate/rebuild: unchanged and reliable.
- Compatibility: no API or storage-layout break.
- Migration complexity: none.
- `CandidatePageIndex` API: unchanged.
- Binary Fuse: remains useful as the optional static page filter.

Option B: hashed marker dictionary, canonical marker string -> keyed hash / opaque ID.

- Private: raw marker strings are no longer stored directly in the dictionary when a key is available.
- Still leaked: equality, frequency, page/index shape, marker reuse patterns, and any unblinded manifest/catalog fields.
- Performance: close to current if hashes are deterministic after unlock.
- Validate/rebuild: must run after unlock and must verify keyed hashes instead of strings.
- Compatibility: old plaintext dictionaries need migration or dual-read support.
- Migration complexity: medium.
- `CandidatePageIndex` API: can likely stay unchanged if `MarkerId` remains the public index unit.
- Binary Fuse: remains useful if filters are built over stable keyed IDs/fingerprints.

Option C: blind marker indexes using keyed marker tokens for catalog and indexes.

- Private: catalog/index files no longer reveal raw marker names and can reduce topic leakage.
- Still leaked: token equality, token frequency, page counts, page sizes, status of page hits, timing, and access patterns.
- Performance: should stay close to current if query markers are converted to keyed tokens once after unlock.
- Validate/rebuild: requires unlock; rebuild must regenerate keyed tokens from decrypted pages or a keyed dictionary.
- Compatibility: existing indexes need rebuild; old plaintext catalog/index files need explicit migration policy.
- Migration complexity: medium to high.
- `CandidatePageIndex` API: should remain unchanged if tokens fit the existing `MarkerId`/u64-style path; do not add a new filter family.
- Binary Fuse: remains useful because Binary Fuse can operate on keyed marker fingerprints instead of raw marker IDs.

Option D: encrypted dictionary with plaintext derived index IDs.

- Private: raw marker strings at rest in the dictionary.
- Still leaked: derived ID equality/frequency, index membership, catalog summaries, page sizes, and any stable ID correlations across backups.
- Performance: near current after unlock because indexes still use plaintext derived IDs.
- Validate/rebuild: dictionary operations require unlock; indexes can still be checked structurally.
- Compatibility: moderate; dictionary read/write path changes, indexes mostly remain stable.
- Migration complexity: medium.
- `CandidatePageIndex` API: unchanged.
- Binary Fuse: remains useful.

Option E: fully encrypted metadata.

- Private: maximum at-rest metadata privacy for dictionary, catalog, summaries, and indexes.
- Still leaked: file count, file sizes, timestamps, access timing, and process-memory data while unlocked.
- Performance: likely much slower unless a new private index design is added; naive mode becomes page full-scan or broad decrypt/decode.
- Validate/rebuild: requires unlock and may become expensive; offline structural validation becomes limited.
- Compatibility: high break risk for tools, smokes, and mixed stores.
- Migration complexity: high.
- `CandidatePageIndex` API: likely needs redesign or an adapter layer.
- Binary Fuse: only useful if rebuilt over keyed/private tokens; otherwise loses its current role.

Recommendation for Memory Genome Engine:

- Keep the current payload-encrypted mode as the default encrypted mode.
- Document plaintext metadata leakage honestly and treat it as an explicit tradeoff, not a hidden bug.
- Add optional blind marker mode later only after a prototype proves recall correctness, rebuild behavior, and benchmark impact.
- Do not fully encrypt all metadata by default; it would push recall toward slow full scans and complicate validation.
- Do not break `CandidatePageIndex`, `MarkerGenome`, recall modes, or the Exact/BinaryFuse strategy.
- Do not add a filter zoo. If blind mode is added, reuse the existing exact/BinaryFuse index boundary with keyed marker IDs or keyed marker fingerprints.

Future phased plan, not implemented now:

- Phase 1: add keyed marker fingerprints for encrypted stores, reducing plaintext dictionary risk while preserving public marker/query input and existing index API.
- Phase 2: move catalog summaries and index files to keyed marker IDs/fingerprints; require unlock for validate/rebuild and rebuild existing indexes explicitly.
- Phase 3: consider an optional higher-privacy mode with reduced pruning or more expensive validation for users who accept slower recall.

Before implementing any blind marker prototype, it must prove:

- no false negatives for focused, broad, or full-scope recall under the existing recall modes;
- `CandidatePageIndex` stays stable and does not introduce a new filter family;
- `ExactMarkerPageIndex` remains the reliable baseline and Binary Fuse remains the only optional static filter backend;
- `validate --deep` and `rebuild-indexes` work after unlock and produce understandable errors when locked;
- migration from plaintext marker metadata is explicit and reversible for test stores;
- benchmark results show acceptable overhead for recall, seal, validate, and rebuild;
- leakage is documented honestly: equality/frequency, page count, page size, timing, and access patterns may still leak.

If those points are not satisfied, keep payload-encrypted mode as the default and leave blind marker metadata as future work.

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

## Future Security Work

Payload encryption is ready for the current product. Future security work is non-blocking and should start only from a separate design:

- optional keyed marker fingerprints / blind marker metadata prototype;
- encrypted indexes or encrypted marker dictionary, only after benchmark and migration evidence;
- encrypted Markdown export mode;
- interactive unlock prompt or host key-management integration.

Do not encrypt indexes, marker dictionary, or catalog summaries by default, because those structures define recall pruning and validation behavior.
