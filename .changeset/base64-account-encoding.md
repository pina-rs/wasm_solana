---
wasm_client_solana: patch
---

# Request base64 when an account encoding is left unset

`get_account_with_config`, `get_multiple_accounts_with_config`, and `get_program_accounts_with_config` decode the response with `UiAccount::to_account`. That only works for a binary encoding. A config that left `encoding` empty (including `..Default::default()` with a commitment filled in, which is how nonce accounts are loaded) omitted the field, and the node answered with `jsonParsed`. The parsed payload has no raw bytes, so a live account came back as `None` and looked missing. `get_multiple_accounts` kept the `None` in place, so the index still matched the request, but the account data was gone.

Unset encodings now go out as `base64`. An encoding the caller set, including `jsonParsed` on the token helpers, is left alone. `RpcAccountInfoConfig::default()` now sets `base64` as well, matching the builder and the field docs.
