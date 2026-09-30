---
memory_wallet: fix
wasm_client_solana: fix
---

# wallet_standard 0.7 lands the browser bridge fixes

Bumps `wallet_standard` from `^0.6` (0.6.0) to `^0.7` (0.7.1). The release lands the pina-rs/wallet_standard#55 fixes: the browser bridge's infinite recursion in `connect_with_options`/`disconnect`, the corrected `sign_transactions` wire format, and the `solana` feature compiling standalone — the wasm-compatibility work this client's browser story depends on. The workspace's own usage of the core crate compiles unchanged on both native (ssr) and wasm32 (`js`); the breaking changes in the release are confined to `wallet_standard_browser` 0.7.0, which this workspace does not depend on.
