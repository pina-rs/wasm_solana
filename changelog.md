# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0](https://github.com/pina-rs/wasm_solana/releases/tag/v0.12.0) (2026-09-16)

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

_Owner:_ Ifiok Jr. · _Introduced in:_ [`60c8807`](https://github.com/pina-rs/wasm_solana/commit/60c88079c6e4f00038a29f39c705218ce579a4e5)

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
