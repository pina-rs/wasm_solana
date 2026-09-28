---
memory_wallet: none
test_utils_insta: none
test_utils_keypairs: none
test_utils_solana: none
wasm_client_solana: none
---

# Ignore three new advisories for the validator stack

`proc-macro-error2 is unmaintained` (RUSTSEC-2026-0173) reaches the lockfile only through `aquamarine ← solana-runtime` — the host-side validator/test harness — and is never part of any published client surface, so it joins the existing validator-stack ignore list. RUSTSEC-2026-0097 (rand unsoundness) hits rand 0.7.3, reachable only through ed25519-dalek 1.x inside agave-precompiles; the client's own rand pins (0.8.8, 0.9.x) are in the patched ranges, so nothing published is affected. RUSTSEC-2026-0292 (imbl-sized-chunks use-after-free) enters through imbl in solana-runtime, again validator-only.
