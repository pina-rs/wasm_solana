---
wasm_client_solana: fix
---

# Pay CI only for what each job uses

The changeset-policy job no longer installs the five workspace cargo binaries it never runs (the devenv action grows an opt-out), the build job compiles the all-features build once instead of twice, and markdown-only changes skip the compile jobs entirely — the separate changeset-policy workflow still gates changeset-only pull requests. The browser wasm test becomes blocking and version-safe: chromium and chromedriver both come from the pinned nixpkgs revision, so a runner Chrome auto-update can no longer break the session handshake invisibly. Benchmarks get a `workflow_dispatch` job so the criterion suites are runnable without a local environment. Also prunes dead workspace dependencies (heck, paste, regex), untracks the machine-specific `.video_agent` path, deduplicates and repairs `.gitignore`, drops deny.toml dead advisories config, removes a zizmor entry for a workflow that never existed, and slims the toolchain profile CI hosts download.
