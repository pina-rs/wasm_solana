---
solana-account-decoder-wasm: fix
solana-transaction-status-wasm: fix
wasm_client_solana: breaking
---

# Live-edge subscription forks, benchmarks, leaner deps

Subscription ack waits and new subscriptions now fork from the live edge of the shared websocket buffer (`fork_stream::Weak` upgrade) instead of cloning the never-read root fork, whose offset pinned them to the buffer's oldest entry. Setup previously replayed and re-parsed the socket's entire history before reaching live traffic. The new `subscription` criterion benchmark measures the ack wait at 73µs / 747µs / 10.1ms for 100 / 1k / 10k buffered frames on the old path, versus 120ns / 317ns / 1.35µs at the live edge — roughly 7,500× at 10k frames, and the gap grows linearly with connection age on busy sockets.

Two new criterion suites (`cargo bench -p wasm_client_solana -F ssr`) baseline the rest of the hot paths: full mock-provider dispatch (balance 1.4µs, one 256B account 3.0µs, 100×1KiB multi-account 237µs ≈ 413 MiB/s, 1000×256B program-accounts 1.8ms ≈ 136 MiB/s), wire primitives (wincode+base64 v0 encode 149ns, base64 1KiB decode 402ns ≈ 2.4 GiB/s, base58 pubkey 467ns), and per-notification parsing (1.5µs/frame).

Also drops four unused dependencies from `wasm_client_solana` — `async-tungstenite` (and with it the tungstenite/sha1/httparse tree plus duplicate getrandom 0.2 and thiserror 1 crates), `heck`, `bv`, and `solana-zk-token-sdk` — shrinking the wasm dependency graph from 658 to 575 unique crates (−13%). The remaining `Inflector` call sites in the account-decoder and transaction-status forks are replaced with a hand-rolled kebab-case helper, removing the `regex` stack from the graph; all 149 fork fixture tests pin the converted program names byte-for-byte. Dead-code elimination meant these unused crates were already stripped from the shipped cdylib, so the wins are compile time and supply-chain surface, not binary size; the regex removal additionally benefits apps that invoke the account parsers.

Known limitation kept for a follow-up: the root fork is retained unread to keep the shared buffer alive, so buffered frames still accumulate for the provider's lifetime; routing notifications through per-subscription-id channels is the remaining fix for buffer retention.
