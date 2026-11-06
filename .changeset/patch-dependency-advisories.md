---
memory_wallet: patch
wasm_client_solana: patch
---

# Patch dependency advisories reachable within existing requirements

Four advisory-affected crates move within their already-declared ranges:

- `tokio` 1.53.1 → 1.54.1 — `AsyncFd::new` and related constructors violate I/O safety (RUSTSEC-2026-0386)
- `quinn-proto` 0.11.18 → 0.11.19 — payload-length and stream-reassembly advisories
- `pulldown-cmark` 0.13.4 → 0.13.6 — exponential event growth with `ENABLE_WIKILINKS` (RUSTSEC-2026-0414, RUSTSEC-2026-0433)
- `libz-sys` 1.1.29 → 1.1.31

Lockfile only. The remaining advisory-affected crates in the graph are either
unmaintained-crate advisories with no patched release (`ansi_term`, `atty`,
`instant`, `net2`, `derivative`, `paste`, `proc-macro-error2`,
`unsafe-libyaml`) or old majors pinned by manifest requirements, both of which
need code changes rather than a lock bump.
