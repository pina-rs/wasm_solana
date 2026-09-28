---
solana-account-decoder-client-types-wasm: fix
wasm_client_solana: fix
---

# Make the `zstd` feature actually decode `base64+zstd` accounts

`wasm_client_solana/zstd` enabled the crate's own unused `zstd` dependency but never armed the decoder forks, so `UiAccountData::decode` kept hitting the non-zstd arm and returned `None` — accounts silently "disappeared" (surfacing as `AccountNotFound`) with no error. The feature now forwards to `solana-account-decoder-wasm/zstd` and `solana-account-decoder-client-types-wasm/zstd`, which matches how the encoder side is gated.

The fork's `base64+zstd` decode also decompressed with no output cap; a tiny compressed blob from a hostile RPC could expand without bound and OOM the process. Decompression is now capped at 10 MiB, matching the cluster's own maximum account data size.
