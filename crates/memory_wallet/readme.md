# `memory_wallet`

<br />

> A memory based wallet standard implementation primarily used for testing.

<br />

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

## Installation

To install you can used the following command:

```bash
cargo add memory_wallet
```

Or directly add the following to your `Cargo.toml`:

```toml
[dependencies]
memory_wallet = "0.1.24"
```

### Features

- `ssr` Enables the `ssr` feature for the `wallet_standard` crate.
- `js` Enables the `js` feature to unlock wasm support for the `wallet_standard` crate.

## Usage

The memory wallet is a simple wallet that stores all accounts in memory and conforms to the `WalletStandard` trait.

```rust
use anyhow::Result;
use memory_wallet::MemoryWallet;
use memory_wallet::prelude::*;
use solana_native_token::sol_str_to_lamports;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signature;
use solana_sdk::system_instruction;
use solana_sdk::transaction::VersionedTransaction;
use wallet_standard::SolanaSignTransactionProps;
use wasm_client_solana::DEVNET;
use wasm_client_solana::SolanaRpcClient;

async fn run() -> Result<()> {
	let keypair = Keypair::new();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let instruction =
		system_instruction::transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.5").unwrap());
	let rpc = SolanaRpcClient::new(DEVNET);
	let blockhash = rpc.get_latest_blockhash().await?;
	let transaction =
		VersionedTransaction::new_unsigned_v0(&pubkey, &[instruction], &[], blockhash)?;
	let mut memory_wallet = MemoryWallet::new(rpc, &[keypair]);

	// connect the first account in the memory wallet accounts list
	memory_wallet.connect().await?;

	let props = SolanaSignTransactionProps::builder()
		.transaction(transaction)
		.build();
	let signed_transaction: VersionedTransaction = memory_wallet.sign_transaction(props).await?;

	Ok(())
}
```

[crate-image]: https://img.shields.io/crates/v/memory_wallet.svg
[crate-link]: https://crates.io/crates/memory_wallet
[docs-image]: https://docs.rs/memory_wallet/badge.svg
[docs-link]: https://docs.rs/memory_wallet/
[ci-status-image]: https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg
[ci-status-link]: https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/ifiokjr/wasm_solana/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/ifiokjr/wasm_solana
