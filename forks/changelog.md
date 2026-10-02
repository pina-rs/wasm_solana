# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## forks [4.0.0](https://github.com/pina-rs/wasm_solana/releases/tag/forks/v4.0.0) (2026-09-08)

Grouped release for `forks`.

### Breaking Changes

#### Update Solana dependencies to Agave 4.x

_Packages:_ _solana-account-decoder-client-types-wasm_, _solana-account-decoder-wasm_, _solana-transaction-status-client-types-wasm_, _solana-transaction-status-wasm_

Move every Solana crate to the latest stable Agave 4.2 family, regenerate the wasm forks from `solana-account-decoder` / `solana-transaction-status` 4.2 sources, depend on `wallet_standard` 0.6, raise the MSRV to 1.89.0 and the pinned toolchain to 1.98.1. The forks re-version to 4.2.0 to mirror the upstream Agave family, and the workspace crates unify on a single release version.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/pina-rs/wasm_solana/pull/128) · _Related issues:_ [#126](https://github.com/pina-rs/wasm_solana/issues/126)

## forks [4.0.1](https://github.com/pina-rs/wasm_solana/releases/tag/forks/v4.0.1) (2026-09-08)

Grouped release for `forks`.

### Fixes

#### Mark the workspace crates as publishable

_Packages:_ _solana-account-decoder-client-types-wasm_, _solana-account-decoder-wasm_, _solana-transaction-status-client-types-wasm_, _solana-transaction-status-wasm_

The v0.11.0 release record had no package publications because every crate manifest still carried `publish = false` from the knope era; crates.io never received the versioned crates. With `publish = true` restored this release publishes the already-versioned crates.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #139](https://github.com/pina-rs/wasm_solana/pull/139)

## forks [4.0.2](https://github.com/pina-rs/wasm_solana/releases/tag/forks/v4.0.2) (2026-09-09)

Grouped release for `forks`.

### Fixes

#### Publish the versioned crates that missed the v0.11.1 release

_Packages:_ _solana-account-decoder-client-types-wasm_, _solana-account-decoder-wasm_, _solana-transaction-status-client-types-wasm_, _solana-transaction-status-wasm_

The v0.11.1 publish published the forks and `wasm_client_solana`, but `memory_wallet` failed because `cargo publish --locked` resolves dev-dependencies and `test_utils_solana ^0.11.1` was not on crates.io yet. The publish order now includes dev-dependencies, so this release publishes the remaining crates.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #142](https://github.com/pina-rs/wasm_solana/pull/142)

## forks [5.0.0](https://github.com/pina-rs/wasm_solana/releases/tag/forks/v5.0.0) (2026-10-02)

Grouped release for `forks`.

### Breaking Changes

#### Document the public API and make the modules public

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #150](https://github.com/pina-rs/wasm_solana/pull/150)

Marked `breaking` because the internal modules of `wasm_client_solana` and `test_utils_solana` become `pub mod` — the lint only enforces documentation inside public module trees, so without this the flat `pub use` re-exports would let hundreds of items escape it. The flat re-exports remain the supported import style, but every item is now also reachable by module path, which is new public API surface.

Enable the `missing_docs` lint across the workspace and document every public item in `wasm_client_solana`, `memory_wallet`, the `test_utils_*` crates and the vendored Agave forks. Add `mdt` templates for documentation that appears in more than one place, so the shared wording cannot drift, and gate it in CI with a new `lint:docs` task. `rustdoc` now builds with no warnings.

#### Upstream client-types and the finished pubsub surface

_Packages:_ 🔴 _solana-account-decoder-wasm_, 🔴 _solana-transaction-status-wasm_

_Owner:_ Ifiok Jr. · _Introduced in:_ [8639311](https://github.com/pina-rs/wasm_solana/commit/86393119f02fefffd42de1541c9b8214ac8f0f35)

Adopt the crates.io `solana-account-decoder-client-types` and `solana-transaction-status-client-types` — upstream 4.2.2 made `zstd` optional in the account one and keeps nothing native-only in either — and retire the two `-client-types-wasm` forks that existed only to provide that. Two published crates disappear from every Agave upgrade cycle; only the full decoder forks remain, because upstream still forces `zstd`, `Inflector`, and (for transaction-status) `solana-entry` on those.

Breaking: the forks re-export the client-types surface, so their public API shifts with the swap (`UiAccount.owner` is now the upstream `String`, the `Eq` impls are gone) — the decoder forks take a major bump even though their own parse logic is untouched. `TestValidatorRunnerProps` also gains the `enable_vote_subscription` field, which is major for exhaustive struct construction.

For `wasm_client_solana`: `UiAccount.owner` is now the upstream `String` rather than the fork's `Pubkey`-typed field, and response types that derived `Eq` over decoder types (`GetTokenAccountBalanceResponse`, `GetTokenSupplyResponse`, `GetTransactionResponse`) keep `PartialEq` only. In exchange the wire types share identity with every other `solana-*` crate in the dependency graph, including the native test stack. The fork's decompression-bomb cap survives as `decode_account_data` in this crate — a hardened replacement for `UiAccountData::decode` that rejects `base64+zstd` payloads expanding past the cluster's own 10 MiB account-data cap.

The native tests also caught a latent ssr pubsub bug: the lazily-connected reqwest websocket polled its handshake future again after it completed (both split halves share it), so any refused connection panicked the provider with `async fn resumed after completion`. The handshake is now latched — a failed connection surfaces as one error frame followed by a permanent end of stream.

`slotsUpdatesSubscribe` and `voteSubscribe` complete the websocket surface: the slot-lifecycle feed arrives as a typed `SlotUpdate` enum tagged by pipeline stage, and the raw vote feed carries `Pubkey`/`Hash`/`Signature` fields instead of base-58 strings. Both are pinned by wire-format tests plus a native integration test driving an in-process validator — which is also why `TestValidatorRunnerProps` gains `enable_vote_subscription`, mirroring the `--rpc-pubsub-enable-vote-subscription` flag production validators keep disabled by default.

### Fixes

#### Live-edge subscription forks, benchmarks, leaner deps

_Packages:_ 🟢 _solana-account-decoder-wasm_, 🟢 _solana-transaction-status-wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #159](https://github.com/pina-rs/wasm_solana/pull/159) · _Related issues:_ [#154](https://github.com/pina-rs/wasm_solana/issues/154)

Subscription ack waits and new subscriptions now fork from the live edge of the shared websocket buffer (`fork_stream::Weak` upgrade) instead of cloning the never-read root fork, whose offset pinned them to the buffer's oldest entry. Setup previously replayed and re-parsed the socket's entire history before reaching live traffic. The new `subscription` criterion benchmark measures the ack wait at 73µs / 747µs / 10.1ms for 100 / 1k / 10k buffered frames on the old path, versus 120ns / 317ns / 1.35µs at the live edge — roughly 7,500× at 10k frames, and the gap grows linearly with connection age on busy sockets.

Two new criterion suites (`cargo bench -p wasm_client_solana -F ssr`) baseline the rest of the hot paths: full mock-provider dispatch (balance 1.4µs, one 256B account 3.0µs, 100×1KiB multi-account 237µs ≈ 413 MiB/s, 1000×256B program-accounts 1.8ms ≈ 136 MiB/s), wire primitives (wincode+base64 v0 encode 149ns, base64 1KiB decode 402ns ≈ 2.4 GiB/s, base58 pubkey 467ns), and per-notification parsing (1.5µs/frame).

Also drops four unused dependencies from `wasm_client_solana` — `async-tungstenite` (and with it the tungstenite/sha1/httparse tree plus duplicate getrandom 0.2 and thiserror 1 crates), `heck`, `bv`, and `solana-zk-token-sdk` — shrinking the wasm dependency graph from 658 to 575 unique crates (−13%). The remaining `Inflector` call sites in the account-decoder and transaction-status forks are replaced with a hand-rolled kebab-case helper, removing the `regex` stack from the graph; all 149 fork fixture tests pin the converted program names byte-for-byte. Dead-code elimination meant these unused crates were already stripped from the shipped cdylib, so the wins are compile time and supply-chain surface, not binary size; the regex removal additionally benefits apps that invoke the account parsers.

Known limitation kept for a follow-up: the root fork is retained unread to keep the shared buffer alive, so buffered frames still accumulate for the provider's lifetime; routing notifications through per-subscription-id channels is the remaining fix for buffer retention.

- **Add monostyle inline annotations.** Style-only pass: adds `monostyle.toml` and inline style annotations across the workspace (no behavior changes), plus a `monostyle check` step in CI so annotations stay in sync with the tool. _Packages:_ 🟢 _solana-account-decoder-wasm_, 🟢 _solana-transaction-status-wasm_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #153](https://github.com/pina-rs/wasm_solana/pull/153)

#### Priority fee estimation and complete pubsub surface

_Packages:_ 🟢 _solana-account-decoder-wasm_, 🟢 _solana-transaction-status-wasm_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #162](https://github.com/pina-rs/wasm_solana/pull/162) · _Related issues:_ [#150](https://github.com/pina-rs/wasm_solana/issues/150), [#154](https://github.com/pina-rs/wasm_solana/issues/154), [#159](https://github.com/pina-rs/wasm_solana/issues/159)

`getPriorityFeeEstimate` is the modern replacement for `getRecentPrioritizationFees`: it prices a specific transaction (or a set of locked accounts) at a chosen urgency percentile instead of reporting raw per-slot fee levels. Three client methods cover transaction-based, configured, and account-based estimation; `get_recent_prioritization_fees*` are now `#[deprecated]` with pointers to the replacements. `getMaxShredInsertSlot` — the highest slot with an inserted shred, leading retransmit — was also missing and is added.

The pubsub surface now matches the docs: `signatureSubscribe` (with optional `receivedNotification` frames), `slotSubscribe`, and `rootSubscribe` join account/logs/program/block. The signature subscription in particular is the push-based alternative to polling `getSignatureStatuses`, enabling one-round-trip send confirmation.

Two websocket correctness fixes from review: subscription and acknowledgement forks are now registered _before_ the subscribe request is sent (a frame racing the handshake could previously slip past a fork created after the send), and `Unsubscription` owns its fork rather than holding a weak live-edge handle (retaining a handle past the provider's drop no longer loses the buffer). The fork benchmark now measures the same acknowledgement wait in both variants with the history drain amortized in setup, matching production.

Also from review: the hardened `decode_account_data` helper rejects `base64+zstd` output beyond the 10 MiB cluster cap instead of silently truncating, the hand-rolled kebab-case helper inserts the hyphen before digit suffixes (`SplToken2022` -> `spl-token-2022`, restoring Inflector parity — regression-tested), and the browser e2e transfer test gets a budget covering both its airdrop and its send.
