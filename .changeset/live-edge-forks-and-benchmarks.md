---
solana-account-decoder-wasm: fix
solana-transaction-status-wasm: fix
wasm_client_solana: fix
---

# Benchmarks for the pubsub and dispatch hot paths

Two criterion suites (`cargo bench -p wasm_client_solana -F ssr`) baseline the hot paths: full mock-provider dispatch (balance 1.4µs, one 256B account 3.0µs, 100×1KiB multi-account 237µs ≈ 413 MiB/s, 1000×256B program-accounts 1.8ms ≈ 136 MiB/s), wire primitives (wincode+base64 v0 encode 149ns, base64 1KiB decode 402ns ≈ 2.4 GiB/s, base58 pubkey 467ns), per-notification parsing (1.5µs/frame), and subscription routing (~1–2µs/frame at 1–64 registered subscriptions). The benchmark numbers caught two real performance bugs before release: subscription setup that replayed the socket's entire history (fixed by attaching at the live edge, then superseded by per-subscription routing), and a generic `send` that deep-cloned every response.

Also drops four unused dependencies from `wasm_client_solana` — `async-tungstenite` (and with it the tungstenite/sha1/httparse tree plus duplicate getrandom 0.2 and thiserror 1 crates), `heck`, `bv`, and `solana-zk-token-sdk` — shrinking the wasm dependency graph from 658 to 575 unique crates (−13%). The remaining `Inflector` call sites in the account-decoder and transaction-status forks are replaced with a hand-rolled kebab-case helper, removing the `regex` stack from the graph; all 149 fork fixture tests pin the converted program names byte-for-byte. Dead-code elimination meant these unused crates were already stripped from the shipped cdylib, so the wins are compile time and supply-chain surface, not binary size; the regex removal additionally benefits apps that invoke the account parsers.
