# test_utils_solana

<br />

> Provides utilities and extensions for testing Solana programs. This includes helpers for setting up a test validator, managing test accounts, and interacting with programs in a test environment. It is designed to be compatible with WASM environments.

<br />

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

To install you can used the following command:

```bash
cargo add --dev test_utils_solana
```

Or directly add the following to your `Cargo.toml`:

```toml
[dev-dependencies]
test_utils_solana = "0.8.0"
```

### Features

| Feature          | Description                                                                 |
| ---------------- | --------------------------------------------------------------------------- |
| `test_validator` | Enables the `test_validator` feature for the `solana_test_validator` crate. |

## Usage

The following requires the `test_validator` feature to be enabled.

```rust
use solana_pubkey::pubkey;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;

async fn run() -> TestValidatorRunner {
	let pubkey = pubkey!("99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ");
	let props = TestValidatorRunnerProps::builder()
		.pubkeys(vec![pubkey]) // pubkeys to fund with an amount of sol each
		.initial_lamports(1_000_000_000) // initial lamports to add to each pubkey account
		.namespace("tests") // namespace to use for the validator client rpc
		.build();

	TestValidatorRunner::run(props).await
}
```

## Guide

When writing tests that use this library, you'll need to use the `#[tokio::test(flavor = "multi_thread")]` attribute on your test functions. This is because the test validator runs in a separate thread, and your test will need to communicate with it asynchronously.

### Using `TestValidatorRunner` for Integration Tests

For integration tests that require a realistic Solana runtime, you can use `TestValidatorRunner`.

```rust
use solana_pubkey::pubkey;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;

#[tokio::test(flavor = "multi_thread")]
async fn my_integration_test() {
	let pubkey = pubkey!("99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ");
	let props = TestValidatorRunnerProps::builder()
		.pubkeys(vec![pubkey]) // Pubkeys to fund
		.initial_lamports(1_000_000_000) // Lamports to fund each pubkey with
		.namespace("my_test") // Namespace for the validator's RPC client
		.build();

	let validator = TestValidatorRunner::run(props).await;
	let rpc_client = validator.rpc();

	// Your test logic here...
	// You can use the rpc_client to interact with the test validator
}
```

### Using `ProgramTest` for Unit Tests

For more lightweight unit tests, you can use `ProgramTest` from `solana-program-test`. This library provides helpers to make it easier to work with.

```rust
use solana_account::Account;
use solana_native_token::sol_str_to_lamports;
use solana_program_test::ProgramTest;
use solana_pubkey::Pubkey;
use test_utils_solana::TestRpcProvider;

#[tokio::test(flavor = "multi_thread")]
async fn my_unit_test() {
	let mut program_test = ProgramTest::new(
		"my_program",   // Your program's name
		my_program::ID, // Your program's ID
		None,           // Or Some(processor)
	);

	let user_pubkey = Pubkey::new_unique();
	program_test.add_account(
		user_pubkey,
		Account {
			lamports: sol_str_to_lamports("1.0").unwrap(),
			..Account::default()
		},
	);

	let ctx = program_test.start_with_context().await;
	let rpc_provider = TestRpcProvider::new(ctx);
	let rpc_client = rpc_provider.to_rpc_client();

	// Your test logic here...
	// You can use the rpc_client to send transactions to your program
}
```

[crate-image]: https://img.shields.io/crates/v/test_utils_solana.svg
[crate-link]: https://crates.io/crates/test_utils_solana
[docs-image]: https://docs.rs/test_utils_solana/badge.svg
[docs-link]: https://docs.rs/test_utils_solana/
[ci-status-image]: https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg
[ci-status-link]: https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/ifiokjr/wasm_solana/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/ifiokjr/wasm_solana
