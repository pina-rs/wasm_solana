# Browser examples

Two framework demos of [`wasm_client_solana`](../readme.md) running in the browser against a local [surfpool](https://surfpool.run) node, with Playwright end-to-end tests proving the full flow: HTTP RPC, websocket pubsub, airdrops, and signing + sending a transfer.

| Example                                | Framework  | Renderer |
| -------------------------------------- | ---------- | -------- |
| [`leptos-surfpool`](./leptos-surfpool) | Leptos 0.8 | CSR      |
| [`dioxus-surfpool`](./dioxus-surfpool) | Dioxus 0.7 | web      |

Both apps share the same behavior: they connect to `http://127.0.0.1:8899` (surfpool's defaults match the agave test-validator: HTTP 8899, websocket 8900, `requestAirdrop` supported), read a balance, subscribe to account notifications, airdrop 1 SOL, and sign + send a 0.1 SOL transfer.

## Why these examples exist

They are the executable counterpart of the crate's `js` feature: everything a browser dApp needs — `getVersion`, `getBalance`, `requestAirdrop`, `getLatestBlockhash`, `sendTransaction`, signature confirmation, and `accountSubscribe` — wired into a real UI framework and asserted by Playwright against a real local RPC backend. When something regresses in the wasm transport, these tests fail in a browser, not just in a unit test.

## Prerequisites

- [devenv](https://devenv.sh) shell for the repository (pinned Rust toolchain, `wasm-bindgen` 0.2.128 via `install:cargo:bin`, and the `getrandom_backend="wasm_js"` rustflags from `.cargo/config.toml`)
- `surfpool` on your `PATH` (`cargo install surfpool` or see [surfpool.run](https://surfpool.run))
- Node.js and Python 3 for Playwright and the static file server

## Build and test

```sh
cd examples
npm install
npx playwright install chromium   # first run only
npm run build                     # builds both apps: cargo + wasm-bindgen
npm test                          # starts surfpool + static servers, runs e2e
```

`playwright.config.ts` starts three servers (surfpool on 8899, the two apps on 4173/4174) and reuses any server that is already listening, so an interactive `surfpool start` session is picked up instead of clashing with it.

To iterate on one app by hand:

```sh
npm run build:leptos
python3 -m http.server 4173 --directory leptos-surfpool/dist
# open http://127.0.0.1:4173
```

## Notes on the build

- The examples are **standalone crates** (each with its own `[workspace]` and lockfile, excluded from the repository workspace) so that framework dependencies never enter the published dependency graph or `cargo-deny` scope of `wasm_client_solana`.
- The wasm is produced with plain `cargo build --release --target
  wasm32-unknown-unknown` plus `wasm-bindgen --target web` — no bundler needed. `wasm-bindgen` is pinned to `0.2.128` in both apps to match the repository's `wasm-bindgen-cli` pin, which is why the shim at `../../.bin/.shims/wasm-bindgen` works out of the box. Override with the `WASM_BINDGEN` environment variable if you use a different version.
- The usual framework tooling works too (`trunk serve` for Leptos, `dx serve --platform web` for Dioxus) — the `index.html` files are written so both paths work.
- `wasm_client_solana` derives the pubsub URL from the HTTP endpoint (`http` → `ws`, port + 1). Surfpool's defaults (`--port 8899`, `--ws-port 8900`) match that convention. If you run surfpool on non-default ports, construct the client with `SolanaRpcClient::new_with_ws_and_commitment` instead.
- The demo keypair is the same throwaway key as `test_utils_keypairs::SECRET_KEY_WALLET`. It exists so the Playwright assertions can be relative (balance before/after) across test runs.
