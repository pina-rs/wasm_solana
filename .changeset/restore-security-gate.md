---
memory_wallet: fix
wasm_client_solana: fix
---

# Restore the security CI gate and patch `rustls`

The MonoChange migration (#130) was stacked on a branch that predated the security tooling in #129, so merging it silently reverted that hardening: the `security` CI job disappeared, `deny.toml` and `.github/zizmor.yml` were deleted, workflow actions were left unpinned, and `devenv.nix` kept calling `security:deny` against the missing policy file.

Restore the gate and the files it needs:

- `security:zizmor` passes again: actions are pinned by commit SHA, the workflow `permissions` are scoped to `contents: read`, checkouts set `persist-credentials: false`, and Dependabot cooldowns are configured.
- `security:deny` has its `deny.toml` policy back.
- `security:audit` uses the validator-stack ignore list with a target-local advisory database, and `rustls` moves to 0.23.45 for RUSTSEC-2026-0285.
- The `wasm_client_solana` `js` build no longer warns about three `ssr`-only imports in `http_provider`.
