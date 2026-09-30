---
solana-account-decoder-wasm: fix
solana-transaction-status-wasm: fix
wasm_client_solana: breaking
---

# Priority fee estimation and complete pubsub surface

`getPriorityFeeEstimate` is the modern replacement for `getRecentPrioritizationFees`: it prices a specific transaction (or a set of locked accounts) at a chosen urgency percentile instead of reporting raw per-slot fee levels. Three client methods cover transaction-based, configured, and account-based estimation; `get_recent_prioritization_fees*` are now `#[deprecated]` with pointers to the replacements. `getMaxShredInsertSlot` — the highest slot with an inserted shred, leading retransmit — was also missing and is added.

The pubsub surface now matches the docs: `signatureSubscribe` (with optional `receivedNotification` frames), `slotSubscribe`, and `rootSubscribe` join account/logs/program/block. The signature subscription in particular is the push-based alternative to polling `getSignatureStatuses`, enabling one-round-trip send confirmation.

Two websocket correctness fixes from review: subscription and acknowledgement forks are now registered _before_ the subscribe request is sent (a frame racing the handshake could previously slip past a fork created after the send), and `Unsubscription` owns its fork rather than holding a weak live-edge handle (retaining a handle past the provider's drop no longer loses the buffer). The fork benchmark now measures the same acknowledgement wait in both variants with the history drain amortized in setup, matching production.

Also from review: the hardened `decode_account_data` helper rejects `base64+zstd` output beyond the 10 MiB cluster cap instead of silently truncating, the hand-rolled kebab-case helper inserts the hyphen before digit suffixes (`SplToken2022` -> `spl-token-2022`, restoring Inflector parity — regression-tested), and the browser e2e transfer test gets a budget covering both its airdrop and its send.
