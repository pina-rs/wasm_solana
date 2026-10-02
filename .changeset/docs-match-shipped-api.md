---
wasm_client_solana: fix
---

# Docs match the shipped API

The book taught a different library: seven subscription methods that never existed, `response.value` on a `get_balance` call that returns `u64`, and a `wss://` URL handed to a constructor that takes HTTP. Every page now matches the real API — including the finished pubsub surface (`slots_updates_subscribe`, `vote_subscribe`), the hardened `decode_account_data` decoder, the two-fork story after the client-types retirement, and a migration note for the breaking 0.13 changes (`UiAccount.owner: String`, the `Eq` removals). Both browser examples are un-broken: their `wasm-bindgen` pins move to 0.2.129 to match the workspace CLI, so the documented build works again, and the crate readme stops claiming 0.10.0. New `no_run` doctests cover the pubsub additions and a running doctest covers `decode_account_data`.
