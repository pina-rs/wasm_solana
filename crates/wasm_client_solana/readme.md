# `wasm_client_solana`

<br />

> A WebAssembly (WASM) compatible client for interacting with the Solana RPC and PubSub APIs. It allows for sending transactions, fetching account data, subscribing to account changes, and more, from within a WASM compaitible environment like the web and serverless functions.

<br />

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
wasm_client_solana = "0.9.0"
```

Or use `cargo add`:

```bash
cargo add wasm_client_solana
```

## Building for WASM

When building your crate for the `wasm32-unknown-unknown` target, you need to set a specific `RUSTFLAGS` environment variable. This is required by the `getrandom` crate, a dependency used for generating random numbers, to correctly function in a WASM environment.

```bash
export RUSTFLAGS='--cfg getrandom_backend="wasm_js"'
```

Alternatively you can add this to your `./.cargo/config.toml` file.

```toml
[target.wasm32-unknown-unknown]
rustflags = [
	"--cfg",
	"get_random_backend=\"wasm_js\"",
]
```

Without this flag, you may encounter compilation errors related to `getrandom`.

## Features

This crate provides the following features:

- `js`: Enables the use of the `wasm-bindgen` crate for the `js` target. This is useful for using the crate in a browser environment.
- `ssr`: Enables the use of the `reqwest` and `tokio` crates for the `ssr` target. This is useful for using the crate in a server or non-browser environment.
- `zstd`: Enables the use of the `zstd` as an encoding format and automatically activates the `ssr` target.

## Usage

The `SolanaRpcClient` provides a wasm compatible client for the [solana rpc](https://solana.com/docs/rpc) and [pubsub](https://solana.com/docs/rpc/websocket) methods.

```rust
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::pubkey;
use wasm_client_solana::ClientResult;
use wasm_client_solana::DEVNET;
use wasm_client_solana::SolanaRpcClient;

async fn run() -> ClientResult<()> {
	let client = SolanaRpcClient::new(DEVNET);
	let address = pubkey!("99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ");

	client
		.request_airdrop(&address, sol_str_to_lamports("1.0").unwrap())
		.await?;
	let account = client.get_account(&address).await?;

	log::info!("account: {account:#?}");

	Ok(())
}
```

[crate-image]: https://img.shields.io/crates/v/wasm_client_solana.svg
[crate-link]: https://crates.io/crates/wasm_client_solana
[docs-image]: https://docs.rs/wasm_client_solana/badge.svg
[docs-link]: https://docs.rs/wasm_client_solana/
[ci-status-image]: https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg
[ci-status-link]: https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/ifiokjr/wasm_solana/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/ifiokjr/wasm_solana
