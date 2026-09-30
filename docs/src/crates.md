# Crates

## Workspace layout

```
wasm_solana/
├── crates/
│   ├── wasm_client_solana/    # the RPC + pubsub client (this is the headline crate)
│   ├── memory_wallet/         # wallet-standard in-memory wallet
│   ├── test_utils_solana/     # validator + ProgramTest utilities
│   ├── test_utils_keypairs/   # fixed keypairs for tests
│   └── test_utils_insta/      # insta snapshot redactions
└── forks/
    ├── solana-account-decoder-wasm/
    └── solana-transaction-status-wasm/
```

## `wasm_client_solana`

The wasm-compatible Solana client.

- `SolanaRpcClient` — the entry point; wraps the provider chosen by your feature flags.
- 50+ typed RPC methods under `methods/`, each with `Request`/`Response` structs that mirror the JSON-RPC spec.
- Pubsub subscriptions returning `Subscription<T>` streams.
- `providers/` — `HttpProvider` (reqwest / ssr) and `WebSocketProvider` (browser / js).
- Re-exports the decoding crates under `solana_account_decoder*` and `solana_transaction_status*` so consumers have one import path: the wire types come from the crates.io `-client-types` crates (wasm-safe since upstream 4.2.2), the parse machinery from the two remaining forks.

### The two transports

|              | `js` (wasm)                   | `ssr` (native)              |
| ------------ | ----------------------------- | --------------------------- |
| HTTP         | browser `fetch` via gloo-net  | `reqwest`                   |
| Pubsub       | browser `WebSocket` (web-sys) | `reqwest-websocket` + tokio |
| Runtime deps | none (promises)               | tokio                       |

## `memory_wallet`

A `wallet_standard`-compliant in-memory wallet. It holds keypairs, signs messages/transactions, can sign _and_ send through the connected `SolanaRpcClient`, and implements the full Solana Wallet Standard feature set. It is designed for tests and prototyping, but is a complete wallet implementation.

## `test_utils_solana`

Boots local Solana environments for integration tests:

- `TestValidatorRunner` — starts a real `solana-test-validator` (agave), wires the faucet, and yields an RPC + websocket URL pair.
- `ProgramTestExtension` — adds programs and accounts to a `solana-program-test` environment with less boilerplate.
- Streams helpers (`log_stream_subscription`, `account_stream_subscription`) for exercising pubsub against a live validator.

## `test_utils_keypairs`

Deterministic, pre-defined keypairs (`get_admin_keypair`, `get_wallet_keypair`, ...) so tests reference stable addresses instead of regenerating them.

## `test_utils_insta`

Snapshot helpers for `insta`: redact dynamic values (signatures, slots, timestamps) so RPC snapshots stay deterministic across runs.

## The `solana-*-wasm` forks

See [The Wasm Forks](./forks.md).
