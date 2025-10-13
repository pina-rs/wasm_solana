# `wasm_client_anchor`

<br />

> A Wasm-compatible, type-safe client for interacting with Anchor programs.

<br />

[![Crate][crate-image]][crate-link] [![Docs][docs-image]][docs-link] [![Status][ci-status-image]][ci-status-link] [![Unlicense][unlicense-image]][unlicense-link] [![codecov][codecov-image]][codecov-link]

This crate provides a client for Anchor programs that can be compiled to WebAssembly. It uses a macro-based system to generate type-safe client structs with a builder pattern for each instruction, making it easy and safe to interact with your on-chain programs from any Rust environment, including browsers.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
wasm_client_anchor = "0.9"
```

### Features

- `js`: Enables `wasm-bindgen` support for use in browser environments.
- `ssr`: Enables `reqwest` and `tokio` support for use in server-side or native environments.

## Compatibility

This crate is compatible with **Anchor v0.32.1**.

## How It Works

The core of this library is a set of macros that you use to generate a client for your specific Anchor program. This process involves two main steps:

1. **Code Generation**: You create a small client crate (or module) where you use the provided macros to generate the program client struct and request builders for each instruction.
2. **Usage**: In your application (e.g., a Yew/Leptos component or a server-side process), you use the generated client to build and send transactions.

### 1. Code Generation

First, define a client for your Anchor program. Let's assume your Anchor program is in a crate named `my_anchor_program`. You would create a new `lib.rs` file for your client like this:

```rust
// In your client crate (e.g., `my_anchor_program_client/src/lib.rs`)

use wasm_client_anchor::create_program_client;
use wasm_client_anchor::create_program_client_macro;

// 1. Generate the main client struct, pointing to your program's ID
create_program_client!(my_anchor_program::ID, MyProgramClient);

// 2. Create a macro that will generate request builders
create_program_client_macro!(my_anchor_program, MyProgramClient);

// 3. For each instruction in your program, generate a request builder.
//    - Use "optional:args" if the instruction struct has no fields.
//    - Omit it if the instruction struct has fields.
my_program_client_request_builder!(Initialize, "optional:args");
my_program_client_request_builder!(DoSomething);
```

### 2. Usage

Once the client is generated, you can use it in your application to interact with the program.

```rust
use memory_wallet::MemoryWallet;
use my_anchor_program_client::IntoMyProgramClient;
use my_anchor_program_client::MyProgramClient;
use solana_sdk::signature::Keypair;
use wasm_client_solana::DEVNET;
use wasm_client_solana::SolanaRpcClient;

async fn run() -> anyhow::Result<()> {
	// Setup your RPC client and a wallet
	let rpc = SolanaRpcClient::new(DEVNET);
	let keypair = Keypair::new();
	let mut wallet = MemoryWallet::new(rpc.clone(), &[keypair]);
	wallet.connect().await?;

	// Instantiate your generated program client
	let program_client: MyProgramClient<_> = MyProgramClient::builder()
		.wallet(wallet.clone())
		.rpc(rpc.clone())
		.build()
		.into(); // Convert from the base AnchorProgram

	// --- Example 1: Call a single instruction ---
	let initialize_request = program_client
		.initialize() // The generated method for the `Initialize` instruction
		.accounts(my_anchor_program::accounts::Initialize {
			user: wallet.pubkey(),
			// ... other accounts
		})
		.build();

	let signature = initialize_request.sign_and_send_transaction().await?;
	println!("Initialize Signature: {}", signature);

	// --- Example 2: Compose multiple instructions ---
	let signer_keypair = Keypair::new();
	let composition_request = program_client
		.do_something() // First instruction
		.args(42) // Set instruction arguments
		.accounts(my_anchor_program::accounts::DoSomething {
			signer: signer_keypair.pubkey(),
			// ...
		})
		.signer(&signer_keypair) // Add extra signers
		.build()
		.compose() // Chain to the next instruction
		.initialize()
		.accounts(my_anchor_program::accounts::Initialize {
			user: wallet.pubkey(),
			// ...
		})
		.build();

	let composed_signature = composition_request.sign_and_send_transaction().await?;
	println!("Composed Signature: {}", composed_signature);

	Ok(())
}
```

[crate-image]: https://img.shields.io/crates/v/wasm_client_anchor.svg
[crate-link]: https://crates.io/crates/wasm_client_anchor
[docs-image]: https://docs.rs/wasm_client_anchor/badge.svg
[docs-link]: https://docs.rs/wasm_client_anchor/
[ci-status-image]: https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg
[ci-status-link]: https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci
[unlicense-image]: https://img.shields.io/badge/license-Unlicence-blue.svg
[unlicense-link]: https://opensource.org/license/unlicense
[codecov-image]: https://codecov.io/github/ifiokjr/wasm_solana/graph/badge.svg?token=87K799Q78I
[codecov-link]: https://codecov.io/github/ifiokjr/wasm_solana
