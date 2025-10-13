# test_utils_anchor

<br />

> Utilities and extensions for testing anchor programs in wasm compatible environments.

<br />

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

To install you can used the following command:

```bash
cargo add --dev test_utils_anchor
```

Or directly add the following to your `Cargo.toml`:

```toml
[dev-dependencies]
test_utils_anchor = "0.2" # replace with the latest version
```

### Features

| Feature          | Description                                                                 |
| ---------------- | --------------------------------------------------------------------------- |
| `test_validator` | Enables the `test_validator` feature for the `solana_test_validator` crate. |

## Compatibility

This crate is compatible with Anchor `v0.32.1` and higher.

## Usage

The primary utility in this crate is the `anchor_processor!` macro. It adapts an Anchor program's entrypoint to be compatible with the `solana-program-test` framework, which is essential for integration testing.

### Setting up an Anchor `ProgramTest`

Here is a complete example of how to set up a test environment for an Anchor program, based on the tests in the `example_client` crate.

```rust
use anyhow::Result;
use example_client::ExampleProgramClient; // Your generated client
use memory_wallet::MemoryWallet;
use solana_sdk::account::Account;
use solana_sdk::native_token::sol_to_lamports;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use test_utils_anchor::anchor_processor;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::ProgramTest;
use test_utils_solana::TestRpcProvider;
use tokio;

// 1. Create a function to set up the test environment.
async fn create_program_test() -> TestRpcProvider {
	let mut program_test = ProgramTest::new(
		"example_program",   // Your program's name
		example_program::ID, // Your program's ID
		// Use the macro to wrap your Anchor program's entrypoint
		anchor_processor!(example_program),
	);

	// 2. Add any necessary accounts for your test.
	let wallet_pubkey = get_wallet_keypair().pubkey();
	program_test.add_account(
		wallet_pubkey,
		Account {
			lamports: sol_to_lamports(1.0),
			..Account::default()
		},
	);

	// 3. Start the test validator.
	let context = program_test.start_with_context().await;
	TestRpcProvider::new(context)
}

// 4. Use the setup function in your tests.
#[tokio::test]
async fn my_anchor_test() -> Result<()> {
	let provider = create_program_test().await;
	let rpc_client = provider.to_rpc_client();
	let wallet_keypair = get_wallet_keypair();
	let mut wallet = MemoryWallet::new(rpc_client.clone(), &[wallet_keypair]);
	wallet.connect().await?;

	// You can now use your program client to interact with the test environment.
	let program_client = ExampleProgramClient::builder()
		.wallet(wallet)
		.rpc(rpc_client)
		.build()
		.into();

	// ... send transactions and make assertions ...

	Ok(())
}
```

[crate-image]: https://img.shields.io/crates/v/test_utils_anchor.svg
[crate-link]: https://crates.io/crates/test_utils_anchor
[docs-image]: https://docs.rs/test_utils_anchor/badge.svg
[docs-link]: https://docs.rs/test_utils_anchor/
[ci-status-image]: https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg
[ci-status-link]: https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/ifiokjr/wasm_solana/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/ifiokjr/wasm_solana
