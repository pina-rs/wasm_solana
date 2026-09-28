# The RPC Client

## Constructor

```rust,ignore
use wasm_client_solana::SolanaRpcClient;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
```

- On **native** (`ssr`) targets the URL is an `http(s)://` endpoint.
- On **wasm** (`js`) targets pass an `http(s)://` URL for RPC; pubsub takes the `wss://` variant.

The client is cheap to clone and methods take `&self`.

## Typed methods

Every RPC method lives in its own module under `methods/` with a `Get<...>Request` struct and a matching response type. The client methods are thin wrappers:

```rust,ignore
// request + response are public types, so you can also send them manually
let request = GetBalanceRequest::new_with_config(pubkey, CommitmentConfig::confirmed());
let response: ClientResponse<GetBalanceResponse> = client.send(request).await?;
```

`ClientResponse<T>` carries `id`, `jsonrpc` and `result` — the raw JSON-RPC envelope — so nothing is hidden from you.

Coverage spans the full read surface of the JSON-RPC spec: accounts, blocks, epochs, inflation, stakes, token accounts, transactions, plus write methods (`send_transaction`, `request_airdrop`, `simulate_transaction`) and helper checks (`is_blockhash_valid`, `minimum_ledger_slot`).

## Commitments

Every method has a `*_with_config` variant accepting `CommitmentConfig`, and config types (`RpcBlockConfig`, ...) expose the standard options (`encoding`, `data_slice`, filters). Encodings map onto `UiAccountEncoding` from the forked decoder crates, including `base64+zstd` on native targets.

## Transaction versions

The client reads and writes legacy, v0 and v1 transactions.

Writes go through the `wincode` wire format, which is version dependent: legacy and v0 place a `short_vec` signature count first, while v1 places the message first behind a `0x81` discriminator and moves the signatures to the tail. Encoding a v1 transaction with a serde format such as `bincode` produces bytes the cluster rejects, so `serialize_and_encode` and `deserialize_and_decode` are generic over `wincode` schemas rather than `serde`.

Reads request `maxSupportedTransactionVersion: 1` by default. The `txv1` feature gate is active on mainnet, and a request that omits the field fails outright when it meets a v1 transaction: `getTransaction` returns `-32015`, and a single v1 transaction rejects a whole `getBlock`. Pass a config with `max_supported_transaction_version: None` to opt out.

Sending a v1 transaction requires explicit compute limits. A v1 message defaults `compute_unit_limit` and `loaded_accounts_data_size_limit` to zero, and a transaction carrying zeros fails with `MaxLoadedAccountsDataSizeExceeded`. `VersionedTransaction::new_unsigned_v1` supplies non-zero defaults; use `new_unsigned_v1_with_config` when you have simulated and know the real values. Simulate once with both limits at their maximum, read `unitsConsumed` and `loadedAccountsDataSize` from the result, and round the data size up to the next 32 KiB page.

Other v1 differences worth knowing:

- Address lookup tables are not supported; up to 64 accounts are listed inline.
- Duplicate account addresses are rejected.
- Compute budget instructions are no-ops. A v1 message carries the compute unit limit, loaded accounts data size limit, heap size and priority fee in the message config instead, which surfaces as `transactionConfig` on the response.
- The priority fee is a total in lamports, not a per-compute-unit price.
- Signatures are a fixed-length array sized by the header rather than a length-prefixed vector.

`get_fee_for_message` accepts legacy, v0 and v1 messages through the `SerializableMessage` trait, so a v1 message can be priced before it is signed.

## Errors

RPC failures surface as `ClientError` with structured contents (`RpcError` with code + message), and Solana transaction errors are typed through `solana_transaction_error` — see `errors.rs` for the full hierarchy. The forked client-types also preserve `OptionSerializer<T>` for fields the RPC may omit, null out, or skip for backwards compatibility.

## Re-exports

The client re-exports the wasm decoder crates under their familiar names:

```rust,ignore
pub use solana_account_decoder_client_types_wasm as solana_account_decoder_client_types;
pub use solana_account_decoder_wasm as solana_account_decoder;
pub use solana_transaction_status_client_types_wasm as solana_transaction_status_client_types;
pub use solana_transaction_status_wasm as solana_transaction_status;
```

This gives consumers a single import path for parsed accounts, transaction statuses and their types, regardless of which forked crate actually hosts them.

## Versioning

The crate versions track the Agave family it targets. The workspace dependency table pins each `solana-*` crate to the range the stable Agave train publishes (see the workspace `Cargo.toml`), so a wasm client compiled against Agave 4.2.x talks to validators of the same family without type drift.
