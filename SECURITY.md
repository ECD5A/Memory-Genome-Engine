# Security Policy

Memory Genome Engine has a full security and threat-model document in [docs/SECURITY.md](docs/SECURITY.md).

## Supported Versions

Security fixes target the current `main` branch and the latest supported GitHub release.

## Reporting A Vulnerability

Use GitHub private vulnerability reporting when it is available for this repository.

If private reporting is not available, open a minimal public issue asking for a private maintainer contact. Do not include exploit details, private stores, passphrases, encrypted payloads, or sensitive corpus data in a public issue.

## Scope

Relevant security reports include:

- encrypted store behavior;
- passphrase handling;
- CLI, JSON-RPC adapter, Python SDK, and TypeScript SDK integration bugs;
- install or release script behavior that could overwrite unexpected paths;
- unsafe plaintext fallback.

Known non-goals are documented in [docs/SECURITY.md](docs/SECURITY.md), including plaintext metadata, process memory exposure while unlocked, and plaintext Markdown export.
