---
memory_wallet: docs
solana-account-decoder-client-types-wasm: fix
solana-account-decoder-wasm: fix
solana-transaction-status-client-types-wasm: fix
solana-transaction-status-wasm: fix
test_utils_insta: docs
test_utils_keypairs: docs
test_utils_solana: docs
wasm_client_solana: fix
---

# Add monostyle inline annotations

Style-only pass: adds `monostyle.toml` and inline style annotations across the workspace (no behavior changes), plus a `monostyle check` step in CI so annotations stay in sync with the tool.
