---
wasm_client_solana: major
memory_wallet: major
test_utils_insta: major
test_utils_keypairs: major
test_utils_solana: major
---

# Support v1 transactions

The `txv1` feature gate (SIMD-0385) is active on mainnet. It raises the transaction size limit to 4096 bytes, moves signatures to the tail behind a `0x81` discriminator, and carries the compute budget in the message rather than in `ComputeBudget` instructions. This client now reads and writes v1 transactions. Sending one previously produced bytes the cluster rejected.

## Sending

`serialize_and_encode` and `deserialize_and_decode` now use the `wincode` wire format instead of `bincode`. The v1 wire layout is not the serde representation of a `VersionedTransaction`, so `bincode` emitted a `short_vec` signature count where the wire requires a version discriminator and placed signatures in the wrong position. Legacy and v0 byte output is unchanged, which is why the existing snapshots still pass.

The `bincode` dependency is dropped from `wasm_client_solana`; `wincode` replaces it.

## Reading

`RpcBlockConfig`, `RpcTransactionConfig` and `RpcBlockSubscribeConfig` default `max_supported_transaction_version` to `1`, and the config-less `get_transaction` now sends a config. Without the field, a v1 transaction makes `getTransaction` return `-32015` and rejects an entire `getBlock`. Use `max_supported_transaction_version: None` to restore the old behavior, or `GetTransactionRequest::new_without_version` for the config-less request. This changes the JSON sent on the wire, so any snapshot of a request built from `Default::default()` will change.

## New APIs

- `VersionedTransaction::new_unsigned_v1` and `new_unsigned_v1_with_config` build v1 transactions. The first sets non-zero compute limits, because a v1 message defaults `compute_unit_limit` and `loaded_accounts_data_size_limit` to zero and a transaction carrying zeros fails with `MaxLoadedAccountsDataSizeExceeded`.
- `SerializableMessage` is implemented for legacy, `v0::Message`, `v1::Message` and `VersionedMessage`. `get_fee_for_message` takes `&impl SerializableMessage`, so it accepts v1 messages; the parameter changed from `&Message`.
- `MAX_SUPPORTED_TRANSACTION_VERSION` and `MAX_LOADED_ACCOUNTS_DATA_SIZE_PER_TRANSACTION` constants.
