---
memory_wallet: none
solana-account-decoder-client-types-wasm: none
solana-account-decoder-wasm: none
solana-transaction-status-client-types-wasm: none
solana-transaction-status-wasm: none
test_utils_insta: none
test_utils_keypairs: none
test_utils_solana: none
wasm_client_solana: none
---

# Add monostyle inline annotations

Style-only pass: adds `monostyle.toml` and inline style annotations across the workspace (no behavior changes), plus a `monostyle check` step in CI so annotations stay in sync with the tool.
