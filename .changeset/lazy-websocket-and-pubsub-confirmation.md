---
wasm_client_solana: minor
---

# Lazy pubsub sockets and push-based confirmation

The browser transport no longer opens a websocket when a client is constructed: `SolanaRpcClient::new` used to open the pubsub socket — and panic through `unwrap_throw` when the endpoint refused it — before the first HTTP call ever ran. The gloo stream now initiates the connection on first use, matching the reqwest backend, so HTTP-only usage never touches pubsub, and a refused or invalid pubsub endpoint surfaces as a single error frame followed by a terminal stream instead of a construction panic.

`confirm_transaction` now confirms through `signatureSubscribe` before falling back to polling: a notification delivers the status the moment the node reaches the target commitment, one covering status poll closes the race with already-confirmed signatures, and the polling loop only runs when pubsub cannot deliver (unavailable endpoint, failed subscription, or silence past the same total budget). `send_and_confirm_transaction*` no longer discards the confirmation result: a transaction that does not confirm — or lands with an on-chain error — now returns an `RpcError` carrying the signature instead of silently reporting success. Callers that treated `Ok` as fire-and-forget acceptance must now check the error, which carries the signature so its status can be re-queried.
