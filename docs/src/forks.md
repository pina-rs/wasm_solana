# The Wasm Forks

Two crates under `forks/` are maintained forks of the official Solana decoder crates. They exist because the upstream versions cannot compile on `wasm32-unknown-unknown`.

| Fork                             | Upstream                    |
| -------------------------------- | --------------------------- |
| `solana-account-decoder-wasm`    | `solana-account-decoder`    |
| `solana-transaction-status-wasm` | `solana-transaction-status` |

The split-out wire-types crates (`solana-account-decoder-client-types`, `solana-transaction-status-client-types`) are consumed **directly from crates.io**: upstream 4.2.2 made `zstd` optional in the account one and gates nothing behind native-only dependencies, so both build for the browser as-is. The two `-client-types-wasm` forks this repo used to publish were retired once that landed. Enable their `agave-unstable-api` feature — despite the name it is only a stability label for the type surface, not a validator feature.

## What the remaining forks change

1. **Optional zstd.** Upstream still makes `zstd` a hard dependency of `solana-account-decoder` (and pulls it transitively into `solana-transaction-status`); the fork restores it as an optional feature with a plain-base64 fallback, because `zstd-sys` cannot build for wasm32-unknown-unknown. Enable the `zstd` feature on native targets.
2. **No `Inflector`.** Both upstream crates depend on `Inflector`, which drags in a regex stack the browser build does not need; the forks use a hand-rolled kebab-case helper (regression-tested against `SplToken2022` -> `spl-token-2022`).
3. **No validator-side dependencies.** Upstream `solana-transaction-status` depends on `solana-entry` and `agave-reserved-account-keys`; the fork drops what a client never uses.
4. **Independent versioning.** The forks carry their own semver train mirroring the Agave family they vendored from (4.0.x from the Agave 4.2 sources). Their docs still link to the upstream crate documentation.

Everything else — wire formats, field names, semantics — is byte-for-byte upstream.

## Re-syncing on an Agave upgrade

When the workspace bumps to a new Agave family, the forks are regenerated from the matching upstream sources:

1. Copy the upstream `src/` over each fork.
2. Re-apply the deltas above (dependency renames to the crates.io client-types crates, the zstd/Inflector gating).
3. Run `dprint fmt` and compile for both native and wasm targets.
4. The RPC response snapshot tests in `wasm_client_solana` pin the wire format — they fail loudly if the fork drifted from the real RPC.

Watch upstream for the retire conditions: once `solana-account-decoder` makes `zstd` optional and drops `Inflector` (and `solana-transaction-status` sheds `solana-entry`), the remaining forks can be deleted the same way the client-types ones were.
