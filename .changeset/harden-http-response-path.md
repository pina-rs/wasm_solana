---
wasm_client_solana: fix
---

# Harden the HTTP response path and `get_multiple_accounts`

- `get_multiple_accounts_with_config` filtered `None` accounts out of the response, so results no longer aligned index-wise with the requested pubkeys — a caller could attribute one account's data to another pubkey. `None` positions are now preserved, the encoding defaults to `base64` (the server's `jsonParsed` default made `UiAccountData::decode` silently return `None`), and the response is consumed instead of deep-cloning every account.
- The generic `send` deserializer deep-cloned the entire response `Value` on every successful call — for `getProgramAccounts`-scale payloads that is a multi-megabyte copy per request, discarded on success. The error shape is now only attempted when the envelope carries a JSON-RPC `error` member.
- Removed the leftover `println!("endpoint: …")` in `SolanaRpcClient::new_with_commitment`: RPC URLs commonly embed provider API keys, and the print leaked them to stdout/logs on every construction.
- Removed the `DEBUG` endpoint constant: it pointed at a third-party plaintext HTTP IP, inviting users to submit signed transactions over an unauthenticated connection.
