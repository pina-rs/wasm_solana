# Testing

## Test suites

| Suite                      | Command                                                               | Needs                    |
| -------------------------- | --------------------------------------------------------------------- | ------------------------ |
| `memory_wallet` (ssr)      | `cargo test_memory_wallet_ssr`                                        | nothing                  |
| `wasm_client_solana` (ssr) | `cargo test_wasm_client_solana_ssr`                                   | nothing (mock transport) |
| doc tests                  | `cargo test_memory_wallet_docs`, `cargo test_wasm_client_solana_docs` | nothing                  |
| streams                    | `cargo test_streams`                                                  | a local validator        |
| surfpool                   | `cargo test_surfpool`                                                 | nothing (embedded node)  |
| wasm (browser)             | `cargo test_wasm`                                                     | validator + chromedriver |

All aliases live in `.cargo/config.toml` and run through `cargo nextest`.

## RPC response snapshots

Every typed method has a `tests::response` snapshot test that deserializes a real RPC JSON payload into the forked response types. These pin the wire format: if a fork drifts from the actual RPC or a type loses a field, the snapshot test fails. This is what makes the fork regeneration process safe.

## Validator-driven tests

`cargo test_streams` boots a real `solana-test-validator` (agave, provisioned through nixpkgs) and exercises pubsub subscriptions end to end — logs and account streams (`tests/streams.rs`), the same feeds through the callback API (`tests/callbacks.rs`), and the pubsub-first confirmation path (`tests/confirm.rs`). The validator lifecycle is managed by `test_utils_solana::TestValidatorRunner`, which also wires the faucet.

## Surfpool-driven tests

`cargo test_surfpool` embeds a [surfpool](https://surfpool.run) node in-process through `test_utils_solana::SurfpoolRunner` (behind the crate's `surfpool` feature) — the same backend the browser e2e examples drive as an external process, but as a library. There is no validator boot or on-disk ledger: the whole suite — startup funding, airdrops, `signatureSubscribe` confirmation, transfers — runs in well under a second. Ports are dynamic and the client defaults to the `Confirmed` commitment, because surfpool's per-transaction block production advances transactions to confirmed without finalizing roots; switch the runner's `block_production_mode` to `Clock` when a test needs finality. Validator-internal behavior (vote accounts, rewards, `getBlock` completeness) still belongs on the agave runner.

## Browser tests

`cargo test_wasm` compiles `wasm_client_solana` for `wasm32-unknown-unknown`, runs `wasm-bindgen-test` through chromedriver, and connects to a background validator:

```bash
test:validator   # starts the validator, runs the browser tests
```

The devenv environment provides `wasm-bindgen-cli` (version-pinned to match the `wasm-bindgen` crate) and `wait-for-them` for port readiness. On Linux it also provides a matched Chromium and chromedriver pair. Run the browser suite in a Linux development shell or CI; the Darwin shell does not install Chromium or chromedriver.

## Coverage

```bash
coverage:all
```

produces `codecov.json` via `cargo llvm-cov` across the SSR suites and streams; CI uploads it to Codecov.

## Browser e2e

The `examples/` directory carries Leptos and Dioxus browser apps driven end-to-end by Playwright against a local surfpool node (`examples/e2e/surfpool.spec.ts`) — the same pubsub flows the wasm test suite covers, exercised in a real browser. See `examples/readme.md` for the build-and-test flow.
