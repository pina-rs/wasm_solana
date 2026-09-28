---
core: breaking
forks: breaking
---

# Document the public API and make the modules public

Marked `breaking` because the internal modules of `wasm_client_solana` and `test_utils_solana` become `pub mod` — the lint only enforces documentation inside public module trees, so without this the flat `pub use` re-exports would let hundreds of items escape it. The flat re-exports remain the supported import style, but every item is now also reachable by module path, which is new public API surface.

Enable the `missing_docs` lint across the workspace and document every public item in `wasm_client_solana`, `memory_wallet`, the `test_utils_*` crates and the vendored Agave forks. Add `mdt` templates for documentation that appears in more than one place, so the shared wording cannot drift, and gate it in CI with a new `lint:docs` task. `rustdoc` now builds with no warnings.
