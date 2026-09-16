---
core: docs
forks: docs
---

# Document every public API item

Enable the `missing_docs` lint across the workspace and document every public item in `wasm_client_solana`, `memory_wallet`, the `test_utils_*` crates and the vendored Agave forks. Add `mdt` templates for documentation that appears in more than one place, so the shared wording cannot drift, and gate it in CI with a new `lint:docs` task. `rustdoc` now builds with no warnings.
