---
wasm_client_solana: fix
---

# Make the zstd feature decode base64+zstd accounts

`wasm_client_solana/zstd` enabled the crate's own unused `zstd` dependency but never armed the decoder, so `base64+zstd` account data kept hitting the non-zstd arm and returned `None` — accounts silently "disappeared" (surfacing as `AccountNotFound`) with no error. The feature now forwards to the upstream `solana-account-decoder-client-types/zstd` and the `solana-account-decoder-wasm/zstd` fork, which matches how the encoder side is gated.

Decompression is also capped: `decode_account_data` (the hardened replacement this crate recommends over the upstream `UiAccountData::decode`) rejects `base64+zstd` payloads that expand past 10 MiB — the cluster's own maximum account data size — so a tiny compressed blob from a hostile RPC can no longer expand without bound and OOM the process.
