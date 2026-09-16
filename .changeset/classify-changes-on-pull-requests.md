---
memory_wallet: none
test_utils_insta: none
test_utils_keypairs: none
test_utils_solana: none
wasm_client_solana: none
solana-account-decoder-client-types-wasm: none
solana-account-decoder-wasm: none
solana-transaction-status-client-types-wasm: none
solana-transaction-status-wasm: none
---

# Classify pull request changes before they reach a release

Add a `changeset-policy` workflow that runs two gates on every pull request.

The changeset policy fails a pull request when changed release-owned paths are not covered by a changeset and posts one remediation comment. It now derives the changed packages from git history with `from: origin/main`, so it also compares each attached changeset bump against the classified change type.

The change-classification step posts the compatibility evidence for the pull request: public API, dependency, and metadata changes with a proposed bump per package. The report separates the current pull request (`proposedChangesetBump`) from everything accumulated since the last release (`releaseFloor`), so a pull request that adds an API is not mistaken for one that breaks it.

Move to monochange 0.13 for the release-aware classification report.

Semantic classification stays off for now. The cargo-semver-checks analyzer cannot build `memory_wallet`, `wasm_client_solana`, or the crates sharing their dependency graph in its isolated baseline, where `solana-message` 4.4.0 pulls a second `wincode` alongside the workspace's pinned version. A failing matrix cell forces `partial` coverage and review on every pull request touching those crates, so classification runs at `--detection-level signature` until the upstream conflict is resolved. The reason is recorded in the workflow next to the `detection-level` input.
