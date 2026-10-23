# Development Workflow

The repository uses [devenv](https://devenv.sh) for a hermetic environment. `direnv` loads it automatically.

## Environment

```bash
devenv shell   # or: direnv allow
```

Provided by the environment: the pinned Rust toolchain (from `rust-toolchain.toml`), agave (validator binaries) and monochange from `ifiokjr/nixpkgs`, mdbook, wasm-bindgen tooling, and all lint/security tools (cargo-deny, cargo-audit, zizmor, gitleaks). Linux also receives the matched Chromium and chromedriver packages used by the browser tests.

## Daily commands

| Command                           | Description                                               |
| --------------------------------- | --------------------------------------------------------- |
| `build:all`                       | Build all crates with all features.                       |
| `test:all`                        | Run every suite, including browser tests (Linux/CI).      |
| `test:validator`                  | Start a local validator and run browser tests (Linux/CI). |
| `coverage:all`                    | llvm-cov coverage across the suites.                      |
| `lint:all`                        | clippy + dprint format check.                             |
| `fix:all`                         | Apply clippy and dprint fixes.                            |
| `security:all`                    | cargo-deny + cargo-audit + zizmor.                        |
| `update:deps`                     | `cargo update` + `devenv update`.                         |
| `build:docs`                      | Build this book.                                          |
| `validator:bg` / `validator:kill` | Manage the local validator.                               |

## Formatting

`dprint` formats everything — Rust via rustfmt nightly, TOML, Markdown, Nix and shell scripts. The pre-commit hook formats changed files; `lint:format` enforces it in CI.

## Git hooks

The devenv git-hooks integration installs gitleaks secret scanning, dprint formatting and nixfmt for Nix files.

## Benchmarking

The `benchmark` workflow runs the criterion suites (`subscription`, `client`) on every pull request: once at the merge-base and once at the PR head, so the numbers isolate exactly what the PR changed. The comparison lands as a PR comment — one per PR, updated in place on every push — and the `benchmark` check fails when any benchmark's median regressed more than ten percent.

A regression is still mergeable when the PR description carries a `## Benchmark Justification` section explaining the tradeoff: the check passes with the section present, and the comment makes the decision visible to reviewers. Make `benchmark` a required status check to enforce that on merge.

Locally, the suites run with:

```sh
cargo bench -p wasm_client_solana -F ssr --bench subscription
cargo bench -p wasm_client_solana -F ssr --bench client
```

## Conventions

- Branch names use Conventional Commit prefixes: `feat/`, `fix/`, `build/`, `ci/`, `docs/`, `chore/`, `test/`.
- Work happens on feature branches with pull requests; CI must pass before merge.
- Commits follow Conventional Commits; release automation derives versions from changesets (see [CI and Releases](./ci-and-releases.md)).
