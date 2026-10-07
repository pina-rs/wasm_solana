---
wasm_client_solana: fix
---

# Surface HTTP status and JSON-RPC error data

A rate-limited 429 or a gateway 502 used to surface as "error decoding response body" — the status check now happens before the body parse, so the failure names the HTTP code on both transports (native reqwest and the browser fetch). `RpcError` also keeps the JSON-RPC `error.data` payload, where a rejected sendTransaction puts the instruction error and the full simulation logs: it is readable via `RpcError::data()` and included in the `Display` output (with the error code) so it survives string logging. Two panic vectors on hostile input close as well: `confirm_transaction` no longer indexes the status array out of bounds, and the pubsub port rewrite refuses to wrap past `u16::MAX` instead of pointing pubsub at an unrelated low port. A bare dependency without the `js` or `ssr` feature now fails with one clear `compile_error!` instead of confusing web-sys errors, and `spawn_local` documents its `LocalSet` panic under `ssr`.
