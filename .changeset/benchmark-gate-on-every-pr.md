---
wasm_client_solana: fix
---

# Benchmark every pull request and gate regressions

A `benchmark` workflow now runs the criterion suites (`subscription` and `client`) on every pull request, twice: once at the merge-base and once at the PR head, so the numbers isolate exactly what the PR changed. The comparison is posted as a PR comment — one comment per PR, updated in place on every push — and to the job's step summary, and the `benchmark` check fails when any benchmark's median regressed more than ten percent. A regression is still mergeable when the PR description carries a `## Benchmark Justification` section explaining the tradeoff, which turns "too slow" into a visible, reviewable decision instead of a silent one; make the job a required status check to enforce that on merge. The comparison itself lives in `setup/scripts/benchmark_compare.py`, pairing the two criterion homes by benchmark id over their median estimates.
