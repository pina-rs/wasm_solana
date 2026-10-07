# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0](https://github.com/pina-rs/wasm_solana/releases/tag/v0.13.0) (2026-10-07)

Grouped release for `core`.

### Breaking Changes

#### Document the public API and make the modules public

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #150](https://github.com/pina-rs/wasm_solana/pull/150)

Marked `breaking` because the internal modules of `wasm_client_solana` and `test_utils_solana` become `pub mod` — the lint only enforces documentation inside public module trees, so without this the flat `pub use` re-exports would let hundreds of items escape it. The flat re-exports remain the supported import style, but every item is now also reachable by module path, which is new public API surface.

Enable the `missing_docs` lint across the workspace and document every public item in `wasm_client_solana`, `memory_wallet`, the `test_utils_*` crates and the vendored Agave forks. Add `mdt` templates for documentation that appears in more than one place, so the shared wording cannot drift, and gate it in CI with a new `lint:docs` task. `rustdoc` now builds with no warnings.

#### Upstream client-types and the finished pubsub surface

_Packages:_ 🔴 _test_utils_solana_, 🔴 _wasm_client_solana_

_Owner:_ Ifiok Jr. · _Introduced in:_ [8639311](https://github.com/pina-rs/wasm_solana/commit/86393119f02fefffd42de1541c9b8214ac8f0f35)

Adopt the crates.io `solana-account-decoder-client-types` and `solana-transaction-status-client-types` — upstream 4.2.2 made `zstd` optional in the account one and keeps nothing native-only in either — and retire the two `-client-types-wasm` forks that existed only to provide that. Two published crates disappear from every Agave upgrade cycle; only the full decoder forks remain, because upstream still forces `zstd`, `Inflector`, and (for transaction-status) `solana-entry` on those.

Breaking: the forks re-export the client-types surface, so their public API shifts with the swap (`UiAccount.owner` is now the upstream `String`, the `Eq` impls are gone) — the decoder forks take a major bump even though their own parse logic is untouched. `TestValidatorRunnerProps` also gains the `enable_vote_subscription` field, which is major for exhaustive struct construction.

For `wasm_client_solana`: `UiAccount.owner` is now the upstream `String` rather than the fork's `Pubkey`-typed field, and response types that derived `Eq` over decoder types (`GetTokenAccountBalanceResponse`, `GetTokenSupplyResponse`, `GetTransactionResponse`) keep `PartialEq` only. In exchange the wire types share identity with every other `solana-*` crate in the dependency graph, including the native test stack. The fork's decompression-bomb cap survives as `decode_account_data` in this crate — a hardened replacement for `UiAccountData::decode` that rejects `base64+zstd` payloads expanding past the cluster's own 10 MiB account-data cap.

The native tests also caught a latent ssr pubsub bug: the lazily-connected reqwest websocket polled its handshake future again after it completed (both split halves share it), so any refused connection panicked the provider with `async fn resumed after completion`. The handshake is now latched — a failed connection surfaces as one error frame followed by a permanent end of stream.

`slotsUpdatesSubscribe` and `voteSubscribe` complete the websocket surface: the slot-lifecycle feed arrives as a typed `SlotUpdate` enum tagged by pipeline stage, and the raw vote feed carries `Pubkey`/`Hash`/`Signature` fields instead of base-58 strings. Both are pinned by wire-format tests plus a native integration test driving an in-process validator — which is also why `TestValidatorRunnerProps` gains `enable_vote_subscription`, mirroring the `--rpc-pubsub-enable-vote-subscription` flag production validators keep disabled by default.

#### Keep websocket subscriptions alive after non-matching frames

_Packages:_ 🔴 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/pina-rs/wasm_solana/pull/154) · _Related issues:_ [#149](https://github.com/pina-rs/wasm_solana/issues/149)

Three websocket code paths returned `Poll::Pending` after consuming a frame that did not match what they were looking for: `Subscription::poll_next` (subscription acks replayed by the shared fork buffer, other subscriptions' notifications, undeserializable frames) and the `filter_map`-based ack waits in `create_subscription` and `Unsubscription::run` (the combinator returns `Pending` after a filtered item without re-polling).

`Forked` only registers the caller's waker with the underlying websocket once its buffer runs dry, so any `Pending` returned while buffered frames remain parked the consuming task with no wakeup armed — the first notification after a subscription ack was never delivered, and un-subscribing replayed history and stalled forever. The browser test for `accountSubscribe` has been silently failing for this reason (the CI wasm step is `continue-on-error`).

All three paths now re-poll in a loop until they find a matching frame, so the buffer drains and the socket waker is re-armed within the same poll. `poll_next` also stops rebuilding an `Unsubscription` on every poll. Verified in a real browser against both `solana-test-validator` and a local surfpool node, plus new Leptos/Dioxus Playwright examples under `examples/`.

#### Live-edge subscription forks, benchmarks, leaner deps

_Packages:_ 🔴 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #159](https://github.com/pina-rs/wasm_solana/pull/159) · _Related issues:_ [#154](https://github.com/pina-rs/wasm_solana/issues/154)

Subscription ack waits and new subscriptions now fork from the live edge of the shared websocket buffer (`fork_stream::Weak` upgrade) instead of cloning the never-read root fork, whose offset pinned them to the buffer's oldest entry. Setup previously replayed and re-parsed the socket's entire history before reaching live traffic. The new `subscription` criterion benchmark measures the ack wait at 73µs / 747µs / 10.1ms for 100 / 1k / 10k buffered frames on the old path, versus 120ns / 317ns / 1.35µs at the live edge — roughly 7,500× at 10k frames, and the gap grows linearly with connection age on busy sockets.

Two new criterion suites (`cargo bench -p wasm_client_solana -F ssr`) baseline the rest of the hot paths: full mock-provider dispatch (balance 1.4µs, one 256B account 3.0µs, 100×1KiB multi-account 237µs ≈ 413 MiB/s, 1000×256B program-accounts 1.8ms ≈ 136 MiB/s), wire primitives (wincode+base64 v0 encode 149ns, base64 1KiB decode 402ns ≈ 2.4 GiB/s, base58 pubkey 467ns), and per-notification parsing (1.5µs/frame).

Also drops four unused dependencies from `wasm_client_solana` — `async-tungstenite` (and with it the tungstenite/sha1/httparse tree plus duplicate getrandom 0.2 and thiserror 1 crates), `heck`, `bv`, and `solana-zk-token-sdk` — shrinking the wasm dependency graph from 658 to 575 unique crates (−13%). The remaining `Inflector` call sites in the account-decoder and transaction-status forks are replaced with a hand-rolled kebab-case helper, removing the `regex` stack from the graph; all 149 fork fixture tests pin the converted program names byte-for-byte. Dead-code elimination meant these unused crates were already stripped from the shipped cdylib, so the wins are compile time and supply-chain surface, not binary size; the regex removal additionally benefits apps that invoke the account parsers.

Known limitation kept for a follow-up: the root fork is retained unread to keep the shared buffer alive, so buffered frames still accumulate for the provider's lifetime; routing notifications through per-subscription-id channels is the remaining fix for buffer retention.

#### Priority fee estimation and complete pubsub surface

_Packages:_ 🔴 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #162](https://github.com/pina-rs/wasm_solana/pull/162) · _Related issues:_ [#150](https://github.com/pina-rs/wasm_solana/issues/150), [#154](https://github.com/pina-rs/wasm_solana/issues/154), [#159](https://github.com/pina-rs/wasm_solana/issues/159)

`getPriorityFeeEstimate` is the modern replacement for `getRecentPrioritizationFees`: it prices a specific transaction (or a set of locked accounts) at a chosen urgency percentile instead of reporting raw per-slot fee levels. Three client methods cover transaction-based, configured, and account-based estimation; `get_recent_prioritization_fees*` are now `#[deprecated]` with pointers to the replacements. `getMaxShredInsertSlot` — the highest slot with an inserted shred, leading retransmit — was also missing and is added.

The pubsub surface now matches the docs: `signatureSubscribe` (with optional `receivedNotification` frames), `slotSubscribe`, and `rootSubscribe` join account/logs/program/block. The signature subscription in particular is the push-based alternative to polling `getSignatureStatuses`, enabling one-round-trip send confirmation.

Two websocket correctness fixes from review: subscription and acknowledgement forks are now registered _before_ the subscribe request is sent (a frame racing the handshake could previously slip past a fork created after the send), and `Unsubscription` owns its fork rather than holding a weak live-edge handle (retaining a handle past the provider's drop no longer loses the buffer). The fork benchmark now measures the same acknowledgement wait in both variants with the history drain amortized in setup, matching production.

Also from review: the hardened `decode_account_data` helper rejects `base64+zstd` output beyond the 10 MiB cluster cap instead of silently truncating, the hand-rolled kebab-case helper inserts the hyphen before digit suffixes (`SplToken2022` -> `spl-token-2022`, restoring Inflector parity — regression-tested), and the browser e2e transfer test gets a budget covering both its airdrop and its send.

### Fixes

- **Add monostyle inline annotations.** Style-only pass: adds `monostyle.toml` and inline style annotations across the workspace (no behavior changes), plus a `monostyle check` step in CI so annotations stay in sync with the tool. _Packages:_ ⚪ _memory_wallet_, ⚪ _test_utils_insta_, ⚪ _test_utils_keypairs_, 🟢 _test_utils_solana_, 🟢 _wasm_client_solana_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #153](https://github.com/pina-rs/wasm_solana/pull/153)
- **Monostyle style pass.** Blank-line breathing room around control flow and returns, group splits, and collapsed blank runs, applied by `monostyle fix` and kept where rustfmt puts them. No behavior change. _Packages:_ 🟢 _memory_wallet_, 🟢 _wasm_client_solana_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #161](https://github.com/pina-rs/wasm_solana/pull/161)
- **wallet_standard 0.7 lands the browser bridge fixes.** Bumps `wallet_standard` from `^0.6` (0.6.0) to `^0.7` (0.7.1). The release lands the pina-rs/wallet_standard#55 fixes: the browser bridge's infinite recursion in `connect_with_options`/`disconnect`, the corrected `sign_transactions` wire format, and the `solana` feature compiling standalone — the wasm-compatibility work this client's browser story depends on. The workspace's own usage of the core crate compiles unchanged on both native (ssr) and wasm32 (`js`); the breaking changes in the release are confined to `wallet_standard_browser` 0.7.0, which this workspace does not depend on. _Packages:_ 🟢 _memory_wallet_, 🟢 _wasm_client_solana_ _Owner:_ Ifiok Jr. · _Introduced in:_ [8639311](https://github.com/pina-rs/wasm_solana/commit/86393119f02fefffd42de1541c9b8214ac8f0f35)
- **Pay CI only for what each job uses.** The changeset-policy job no longer installs the five workspace cargo binaries it never runs (the devenv action grows an opt-out), the build job compiles the all-features build once instead of twice, and markdown-only changes skip the compile jobs entirely — the separate changeset-policy workflow still gates changeset-only pull requests. The browser wasm test becomes blocking and version-safe: chromium and chromedriver both come from the pinned nixpkgs revision, so a runner Chrome auto-update can no longer break the session handshake invisibly. Benchmarks get a `workflow_dispatch` job so the criterion suites are runnable without a local environment. Also prunes dead workspace dependencies (heck, paste, regex), untracks the machine-specific `.video_agent` path, deduplicates and repairs `.gitignore`, removes a zizmor entry for a workflow that never existed, and slims the toolchain profile CI hosts download. _Packages:_ 🟢 _wasm_client_solana_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #174](https://github.com/pina-rs/wasm_solana/pull/174)

#### Docs match the shipped API

_Packages:_ 🟢 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #172](https://github.com/pina-rs/wasm_solana/pull/172)

The book taught a different library: seven subscription methods that never existed, `response.value` on a `get_balance` call that returns `u64`, and a `wss://` URL handed to a constructor that takes HTTP. Every page now matches the real API — including the finished pubsub surface (`slots_updates_subscribe`, `vote_subscribe`), the hardened `decode_account_data` decoder, the two-fork story after the client-types retirement, and a migration note for the breaking 0.13 changes (`UiAccount.owner: String`, the `Eq` removals). Both browser examples are un-broken: their `wasm-bindgen` pins move to 0.2.129 to match the workspace CLI, so the documented build works again, and the crate readme stops claiming 0.10.0. New `no_run` doctests cover the pubsub additions and a running doctest covers `decode_account_data`.

#### Harden the HTTP response path and `get_multiple_accounts`

_Packages:_ 🟢 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/pina-rs/wasm_solana/pull/154) · _Related issues:_ [#149](https://github.com/pina-rs/wasm_solana/issues/149)

- `get_multiple_accounts_with_config` filtered `None` accounts out of the response, so results no longer aligned index-wise with the requested pubkeys — a caller could attribute one account's data to another pubkey. `None` positions are now preserved, the encoding defaults to `base64` (the server's `jsonParsed` default made `UiAccountData::decode` silently return `None`), and the response is consumed instead of deep-cloning every account.
- The generic `send` deserializer deep-cloned the entire response `Value` on every successful call — for `getProgramAccounts`-scale payloads that is a multi-megabyte copy per request, discarded on success. The error shape is now only attempted when the envelope carries a JSON-RPC `error` member.
- Removed the leftover `println!("endpoint: …")` in `SolanaRpcClient::new_with_commitment`: RPC URLs commonly embed provider API keys, and the print leaked them to stdout/logs on every construction.
- Removed the `DEBUG` endpoint constant: it pointed at a third-party plaintext HTTP IP, inviting users to submit signed transactions over an unauthenticated connection.

- **Surface HTTP status and JSON-RPC error data.** A rate-limited 429 or a gateway 502 used to surface as "error decoding response body" — the status check now happens before the body parse, so the failure names the HTTP code on both transports (native reqwest and the browser fetch). `RpcError` also keeps the JSON-RPC `error.data` payload, where a rejected sendTransaction puts the instruction error and the full simulation logs: it is readable via `RpcError::data()` and included in the `Display` output (with the error code) so it survives string logging. Two panic vectors on hostile input close as well: `confirm_transaction` no longer indexes the status array out of bounds, and the pubsub port rewrite refuses to wrap past `u16::MAX` instead of pointing pubsub at an unrelated low port. A bare dependency without the `js` or `ssr` feature now fails with one clear `compile_error!` instead of confusing web-sys errors, and `spawn_local` documents its `LocalSet` panic under `ssr`. _Packages:_ 🟢 _wasm_client_solana_ _Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #173](https://github.com/pina-rs/wasm_solana/pull/173)

#### Make the zstd feature decode base64+zstd accounts

_Packages:_ 🟢 _wasm_client_solana_

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #154](https://github.com/pina-rs/wasm_solana/pull/154) · _Related issues:_ [#149](https://github.com/pina-rs/wasm_solana/issues/149)

`wasm_client_solana/zstd` enabled the crate's own unused `zstd` dependency but never armed the decoder, so `base64+zstd` account data kept hitting the non-zstd arm and returned `None` — accounts silently "disappeared" (surfacing as `AccountNotFound`) with no error. The feature now forwards to the upstream `solana-account-decoder-client-types/zstd` and the `solana-account-decoder-wasm/zstd` fork, which matches how the encoder side is gated.

Decompression is also capped: `decode_account_data` (the hardened replacement this crate recommends over the upstream `UiAccountData::decode`) rejects `base64+zstd` payloads that expand past 10 MiB — the cluster's own maximum account data size — so a tiny compressed blob from a hostile RPC can no longer expand without bound and OOM the process.

## [0.12.1](https://github.com/pina-rs/wasm_solana/releases/tag/v0.12.1) (2026-09-28)

Grouped release for `core`.

### Fixes

- **test_utils_solana**: **Fix a port-picker overflow near u16::MAX.** `TestValidatorPorts::random_ports` drew its base port up to `u16::MAX - 25`, but the gossip range end is `port + 103`; a draw in the top ~80 values overflowed and panicked under debug overflow checks, failing test runs at random. _Owner:_ Ifiok Jr. · _Introduced in:_ [2b61d2f](https://github.com/pina-rs/wasm_solana/commit/2b61d2f154d956e54490996ac9b7e61d059f1c75)

### Notes

- _Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_ **Ignore three new advisories for the validator stack.** `proc-macro-error2 is unmaintained` (RUSTSEC-2026-0173) reaches the lockfile only through `aquamarine ← solana-runtime` — the host-side validator/test harness — and is never part of any published client surface, so it joins the existing validator-stack ignore list. RUSTSEC-2026-0097 (rand unsoundness) hits rand 0.7.3, reachable only through ed25519-dalek 1.x inside agave-precompiles; the client's own rand pins (0.8.8, 0.9.x) are in the patched ranges, so nothing published is affected. RUSTSEC-2026-0292 (imbl-sized-chunks use-after-free) enters through imbl in solana-runtime, again validator-only. _Owner:_ Ifiok Jr. · _Introduced in:_ [9e7f3f7](https://github.com/pina-rs/wasm_solana/commit/9e7f3f739a4a833014d7940247feaf4a22bbb1fe) · _Last updated in:_ [d1e87f6](https://github.com/pina-rs/wasm_solana/commit/d1e87f6f0e144f79b1282ab90a729f0f65e30e73)
- **memory_wallet**: **Poll getTransaction in the v1 sign-and-send test.** Test-only: the single-shot `getTransaction` read raced the completed-block store on slow CI runners (statuses reported the transaction confirmed before the block store could serve it), so the test has failed every CI run since the v1 support landed. It now polls for the same window the confirmation loop uses. _Owner:_ Ifiok Jr. · _Introduced in:_ [6c095e1](https://github.com/pina-rs/wasm_solana/commit/6c095e1b1a57323024c9d2107cc8464644a353fa)

## [0.12.0](https://github.com/pina-rs/wasm_solana/releases/tag/v0.12.0) (2026-09-28)

Grouped release for `core`.

### Breaking Changes

#### Support v1 transactions

_Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_

The `txv1` feature gate (SIMD-0385) is active on mainnet. It raises the transaction size limit to 4096 bytes, moves signatures to the tail behind a `0x81` discriminator, and carries the compute budget in the message rather than in `ComputeBudget` instructions. This client now reads and writes v1 transactions. Sending one previously produced bytes the cluster rejected.

##### Sending

`serialize_and_encode` and `deserialize_and_decode` now use the `wincode` wire format instead of `bincode`. The v1 wire layout is not the serde representation of a `VersionedTransaction`, so `bincode` emitted a `short_vec` signature count where the wire requires a version discriminator and placed signatures in the wrong position. Legacy and v0 byte output is unchanged, which is why the existing snapshots still pass.

The `bincode` dependency is dropped from `wasm_client_solana`; `wincode` replaces it.

##### Reading

`RpcBlockConfig`, `RpcTransactionConfig` and `RpcBlockSubscribeConfig` default `max_supported_transaction_version` to `1`, and the config-less `get_transaction` now sends a config. Without the field, a v1 transaction makes `getTransaction` return `-32015` and rejects an entire `getBlock`. Use `max_supported_transaction_version: None` to restore the old behavior, or `GetTransactionRequest::new_without_version` for the config-less request. This changes the JSON sent on the wire, so any snapshot of a request built from `Default::default()` will change.

##### New APIs

- `VersionedTransaction::new_unsigned_v1` and `new_unsigned_v1_with_config` build v1 transactions. The first sets non-zero compute limits, because a v1 message defaults `compute_unit_limit` and `loaded_accounts_data_size_limit` to zero and a transaction carrying zeros fails with `MaxLoadedAccountsDataSizeExceeded`.
- `SerializableMessage` is implemented for legacy, `v0::Message`, `v1::Message` and `VersionedMessage`. `get_fee_for_message` takes `&impl SerializableMessage`, so it accepts v1 messages; the parameter changed from `&Message`.
- `MAX_SUPPORTED_TRANSACTION_VERSION` and `MAX_LOADED_ACCOUNTS_DATA_SIZE_PER_TRANSACTION` constants.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #149](https://github.com/pina-rs/wasm_solana/pull/149)

### Fixes

#### Restore the security CI gate and patch `rustls`

_Packages:_ _memory_wallet_, _wasm_client_solana_

The MonoChange migration (#130) was stacked on a branch that predated the security tooling in #129, so merging it silently reverted that hardening: the `security` CI job disappeared, `deny.toml` and `.github/zizmor.yml` were deleted, workflow actions were left unpinned, and `devenv.nix` kept calling `security:deny` against the missing policy file.

Restore the gate and the files it needs:

- `security:zizmor` passes again: actions are pinned by commit SHA, the workflow `permissions` are scoped to `contents: read`, checkouts set `persist-credentials: false`, and Dependabot cooldowns are configured.
- `security:deny` has its `deny.toml` policy back.
- `security:audit` uses the validator-stack ignore list with a target-local advisory database, and `rustls` moves to 0.23.45 for RUSTSEC-2026-0285.
- The `wasm_client_solana` `js` build no longer warns about three `ssr`-only imports in `http_provider`.

_Owner:_ Ifiok Jr. · _Introduced in:_ [60c8807](https://github.com/pina-rs/wasm_solana/commit/60c88079c6e4f00038a29f39c705218ce579a4e5)

### Notes

#### Classify pull request changes before they reach a release

_Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_

Add a `changeset-policy` workflow that runs two gates on every pull request.

The changeset policy fails a pull request when changed release-owned paths are not covered by a changeset and posts one remediation comment. It now derives the changed packages from git history with `from: origin/main`, so it also compares each attached changeset bump against the classified change type.

The change-classification step posts the compatibility evidence for the pull request: public API, dependency, and metadata changes with a proposed bump per package. The report separates the current pull request (`proposedChangesetBump`) from everything accumulated since the last release (`releaseFloor`), so a pull request that adds an API is not mistaken for one that breaks it.

Move to monochange 0.13 for the release-aware classification report.

Semantic classification stays off for now. The cargo-semver-checks analyzer cannot build `memory_wallet`, `wasm_client_solana`, or the crates sharing their dependency graph in its isolated baseline, where `solana-message` 4.4.0 pulls a second `wincode` alongside the workspace's pinned version. A failing matrix cell forces `partial` coverage and review on every pull request touching those crates, so classification runs at `--detection-level signature` until the upstream conflict is resolved. The reason is recorded in the workflow next to the `detection-level` input.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #148](https://github.com/pina-rs/wasm_solana/pull/148)

## [0.11.2](https://github.com/pina-rs/wasm_solana/releases/tag/v0.11.2) (2026-09-09)

Grouped release for `core`.

### Fixes

#### Publish the versioned crates that missed the v0.11.1 release

_Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_

The v0.11.1 publish published the forks and `wasm_client_solana`, but `memory_wallet` failed because `cargo publish --locked` resolves dev-dependencies and `test_utils_solana ^0.11.1` was not on crates.io yet. The publish order now includes dev-dependencies, so this release publishes the remaining crates.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #142](https://github.com/pina-rs/wasm_solana/pull/142)

## [0.11.1](https://github.com/pina-rs/wasm_solana/releases/tag/v0.11.1) (2026-09-08)

Grouped release for `core`.

### Fixes

#### Mark the workspace crates as publishable

_Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_

The v0.11.0 release record had no package publications because every crate manifest still carried `publish = false` from the knope era; crates.io never received the versioned crates. With `publish = true` restored this release publishes the already-versioned crates.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #139](https://github.com/pina-rs/wasm_solana/pull/139)

## [0.11.0](https://github.com/pina-rs/wasm_solana/releases/tag/v0.11.0) (2026-09-08)

Grouped release for `core`.

### Breaking Changes

#### Update Solana dependencies to Agave 4.x

_Packages:_ _memory_wallet_, _test_utils_insta_, _test_utils_keypairs_, _test_utils_solana_, _wasm_client_solana_

Move every Solana crate to the latest stable Agave 4.2 family, regenerate the wasm forks from `solana-account-decoder` / `solana-transaction-status` 4.2 sources, depend on `wallet_standard` 0.6, raise the MSRV to 1.89.0 and the pinned toolchain to 1.98.1. The forks re-version to 4.2.0 to mirror the upstream Agave family, and the workspace crates unify on a single release version.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/pina-rs/wasm_solana/pull/128) · _Related issues:_ [#126](https://github.com/pina-rs/wasm_solana/issues/126)

### Fixes

#### Remove unused runtime dependencies from the wasm graph

_Packages:_ _wasm_client_solana_

Drop `solana-compute-budget` and `solana-system-program` from `wasm_client_solana`: they were unused and pulled `solana-program-runtime`, which cannot compile on wasm32-unknown-unknown.

_Owner:_ [@ifiokjr](https://github.com/ifiokjr) · _Review:_ [PR #128](https://github.com/pina-rs/wasm_solana/pull/128)
