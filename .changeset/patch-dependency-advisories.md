---
memory_wallet: patch
wasm_client_solana: patch
---

# Patch dependency advisories within declared ranges

Lockfile-only bump for this workspace. Four crates move within ranges the manifests already declare:

- `tokio` 1.53.1 → 1.54.1 — `AsyncFd::new` and related constructors violate I/O safety (RUSTSEC-2026-0386)
- `pulldown-cmark` 0.13.4 → 0.13.6 — two distinct fixes: exponential event growth with `ENABLE_WIKILINKS` (RUSTSEC-2026-0414, fixed 0.13.5) and quadratic parsing of nested image wikilinks (RUSTSEC-2026-0433, fixed 0.13.6)
- `zstd-safe` 7.3.0 → 7.4.0 — clears the batch of zstd-safe advisories published as RUSTSEC-2026-0395 through 2026-0406
- `libz-sys` 1.1.29 → 1.1.31 — bundled zlib advisory

`quinn-proto` 0.11.18 → 0.11.19 rides along for currency; OSV lists no open advisories at either version. The unbounded stream-reassembly advisory (RUSTSEC-2026-0185) was fixed at 0.11.15 and this lock was already past it.

## Consumers must update their own lockfiles

`memory_wallet` and `wasm_client_solana` are published libraries. This bump hardens _this workspace's_ builds only — the manifests still declare caret ranges (for example `tokio ^1`), so a consumer's lockfile can legitimately resolve `tokio 1.53.1` and remain exposed to RUSTSEC-2026-0386. Anyone consuming these crates should run `cargo update` in their own workspace and audit their lock independently.

The remaining advisory-affected crates in this graph are unmaintained-crate advisories with no patched release (`ansi_term`, `atty`, `instant`, `net2`, `derivative`, `paste`, `proc-macro-error2`, `unsafe-libyaml`) or old majors pinned by manifest requirements — both need code changes rather than a lock bump.
