# CI and Releases

## CI

Every pull request runs (`.github/workflows/ci.yml`):

| Job        | What it does                                                       |
| ---------- | ------------------------------------------------------------------ |
| `lint`     | clippy, monochange metadata and dprint format check.               |
| `security` | cargo-audit, cargo-deny, zizmor.                                   |
| `test`     | SSR suites, doc tests, streams (live validator) and browser tests. |
| `coverage` | llvm-cov report uploaded to Codecov.                               |
| `build`    | locked build for native and `wasm32-unknown-unknown`.              |

All actions are pinned by SHA, checkouts drop credentials, and the workflow runs with `contents: read` only.

## Release flow with MonoChange

Releases are planned by [MonoChange](https://github.com/pina-rs/monochange):

1. **Create a changeset** (`release:change` devenv script or a hand-written `.changeset/*.md`):

   ```markdown
   ---
   wasm_client_solana: minor
   ---

   # Add block subscription filters

   ...
   ```

   The front matter lists packages and their change type; the body becomes the changelog entry.

2. **Release PR** — when changesets land on `main`, the `release-pr` workflow runs `monochange run release`, which computes versions, updates manifests and `Cargo.lock`, refreshes `changelog.md`, and opens the `monochange/release/*` pull request.

3. **Publish** — merging the release PR tags the release, creates the draft GitHub release and dispatches the `publish` workflow, which publishes every package in dependency order using crates.io **trusted publishing** (OIDC, `environment: publisher`). No registry tokens are stored in the repository.

The five workspace crates version together as one release group (they share `workspace.package.version`), so the client and test utilities always ship as a matched set. The four `solana-*-wasm` forks version independently (group `forks`): they carry their own semver train so an Agave family bump can move them a full major (3.0.x → 4.0.0) while the workspace crates bump separately. Both groups publish in the same release flow and publish order respects the dependency graph (client-types forks first, then the decoders, then the client).

## Publishing locally

If CI publishing fails or you need to publish from a machine with credentials:

```bash
# prepare versions, changelog and lockfile, then commit locally
release:local

# publish from the local release commit with local cargo credentials
publish:local
```

## Versioning rules

| Changeset type       | Version effect (pre-1.0) |
| -------------------- | ------------------------ |
| `feat`               | patch bump               |
| `fix`                | patch bump               |
| `breaking` / `major` | minor bump (0.5 → 0.6)   |
| `docs` / `none`      | no release by itself     |

MonoChange applies the standard pre-1.0 bump shifting: breaking changes bump the minor component until the crates reach 1.0.
