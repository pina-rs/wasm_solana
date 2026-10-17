---
test_utils_solana: minor
---

# Embed a surfpool node for tests

`test_utils_solana` grows a `surfpool` feature: `SurfpoolRunner` embeds a surfpool node in-process via the `surfpool-sdk` crate (1.6, published by the Solana Foundation), the same backend the Playwright browser examples already drive externally. Where `TestValidatorRunner` pays for agave genesis, a RocksDB ledger and a faucet thread, a `SurfpoolRunner` starts in milliseconds — its suites boot, airdrop, confirm over pubsub, and send a transfer in under half a second.

The props mirror `TestValidatorRunnerProps` where the backends agree — funded `pubkeys` with `initial_lamports`, seeded `accounts`, deployed `programs`, and the client `commitment` — plus surfpool-specific knobs: `slot_time_ms`, `skip_blockhash_check`, `block_production_mode` and `remote_rpc_url` for lazily forking mainnet state. Ports are always dynamic (`pubsub_url` is _not_ rpc port + 1 on a surfnet), and the client defaults to `Confirmed` because per-transaction block mode advances transactions to confirmed without finalizing roots; the docs on the props call out switching to `Clock` production when a test genuinely needs finality. The agave runner remains the fidelity backend for validator internals.
