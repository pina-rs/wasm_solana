---
memory_wallet: none
test_utils_insta: none
test_utils_keypairs: none
test_utils_solana: none
wasm_client_solana: none
---

# Ignore RUSTSEC-2026-0173 for the validator stack

`proc-macro-error2 is unmaintained` (RUSTSEC-2026-0173) reaches the lockfile only through `aquamarine ← solana-runtime` — the host-side validator/test harness — and is never part of any published client surface, so it joins the existing validator-stack ignore list.
