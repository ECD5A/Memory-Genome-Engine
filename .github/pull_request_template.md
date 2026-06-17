## Summary

<!-- What changed and why? -->

## Scope

- [ ] Core storage / recall
- [ ] CLI / TUI
- [ ] MCP / SDK
- [ ] Security
- [ ] Release / packaging
- [ ] Documentation

## Safety Checklist

- [ ] I did not change the storage layout without a design rationale.
- [ ] I did not change recall semantics without tests.
- [ ] I did not add a new filter family without benchmark evidence.
- [ ] I did not use JSON/JSONL as runtime storage.
- [ ] I did not commit generated stores, target artifacts, passphrases, logs, or private corpus data.

## Verification

<!-- List exact commands run. -->

- [ ] `cargo fmt --check`
- [ ] `cargo test`
- [ ] `cargo check -p mge-cli --bins`
- [ ] Release / smoke checks if packaging or integration changed

## Notes

<!-- Known limitations, skipped checks, screenshots, or follow-up work. -->
