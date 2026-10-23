---
core: fix
forks: fix
---

# Update constant-time dependency to patched ctutils

Refresh the Cargo lockfile to ctutils 0.4.4, fixing RUSTSEC-2026-0385 in constant-time mask generation and u128 comparisons. Public client APIs are unchanged.

Normalize existing Rust comment wrapping with the current formatter; the forked crates have no behavioral change.
