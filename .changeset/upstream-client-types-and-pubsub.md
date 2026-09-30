---
solana-account-decoder-wasm: breaking
solana-transaction-status-wasm: breaking
test_utils_solana: breaking
wasm_client_solana: breaking
---

# Upstream client-types and the finished pubsub surface

Adopt the crates.io `solana-account-decoder-client-types` and `solana-transaction-status-client-types` — upstream 4.2.2 made `zstd` optional in the account one and keeps nothing native-only in either — and retire the two `-client-types-wasm` forks that existed only to provide that. Two published crates disappear from every Agave upgrade cycle; only the full decoder forks remain, because upstream still forces `zstd`, `Inflector`, and (for transaction-status) `solana-entry` on those.

Breaking: the forks re-export the client-types surface, so their public API shifts with the swap (`UiAccount.owner` is now the upstream `String`, the `Eq` impls are gone) — the decoder forks take a major bump even though their own parse logic is untouched. `TestValidatorRunnerProps` also gains the `enable_vote_subscription` field, which is major for exhaustive struct construction.

For `wasm_client_solana`: `UiAccount.owner` is now the upstream `String` rather than the fork's `Pubkey`-typed field, and response types that derived `Eq` over decoder types (`GetTokenAccountBalanceResponse`, `GetTokenSupplyResponse`, `GetTransactionResponse`) keep `PartialEq` only. In exchange the wire types share identity with every other `solana-*` crate in the dependency graph, including the native test stack. The fork's decompression-bomb cap survives as `decode_account_data` in this crate — a hardened replacement for `UiAccountData::decode` that rejects `base64+zstd` payloads expanding past the cluster's own 10 MiB account-data cap.

The native tests also caught a latent ssr pubsub bug: the lazily-connected reqwest websocket polled its handshake future again after it completed (both split halves share it), so any refused connection panicked the provider with `async fn resumed after completion`. The handshake is now latched — a failed connection surfaces as one error frame followed by a permanent end of stream.

`slotsUpdatesSubscribe` and `voteSubscribe` complete the websocket surface: the slot-lifecycle feed arrives as a typed `SlotUpdate` enum tagged by pipeline stage, and the raw vote feed carries `Pubkey`/`Hash`/`Signature` fields instead of base-58 strings. Both are pinned by wire-format tests plus a native integration test driving an in-process validator — which is also why `TestValidatorRunnerProps` gains `enable_vote_subscription`, mirroring the `--rpc-pubsub-enable-vote-subscription` flag production validators keep disabled by default.
