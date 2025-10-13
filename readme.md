<p align="center">
  <a href="#">
    <img width="300" height="300" src="./setup/assets/logo.svg"  />
  </a>
</p>

<p align="center">
  solana development with a rust based wasm client
</p>

<p align="center">
  <a href="https://github.com/ifiokjr/wasm_solana/actions?query=workflow:ci">
    <img src="https://github.com/ifiokjr/wasm_solana/workflows/ci/badge.svg" alt="Continuous integration badge for github actions" title="CI Badge" />
  </a>
</p>

<br />

## Description

This repository contains several crates that make it easier to interact with Solana in WebAssembly environments:

| Crate                                                 | Version | Description                                                               |
| ----------------------------------------------------- | ------- | ------------------------------------------------------------------------- |
| [`memory_wallet`](./crates/memory_wallet)             | 0.1     | A memory based wallet standard implementation primarily used for testing. |
| [`test_utils_insta`](./crates/test_utils_insta)       | 0.1     | Test utilities for working with `insta` redactions                        |
| [`test_utils_keypairs`](./crates/test_utils_keypairs) | 0.1     | Test utilities for working with pre-defined keypairs                      |
| [`test_utils_solana`](./crates/test_utils_solana)     | 0.5     | Testing utilities for Solana programs                                     |
| [`test_utils_anchor`](./crates/test_utils_anchor)     | 0.2     | Testing utilities specific to Anchor programs                             |
| [`wasm_client_anchor`](./crates/wasm_client_anchor)   | 0.9     | WebAssembly client for interacting with Anchor programs                   |
| [`wasm_client_solana`](./crates/wasm_client_solana)   | 0.9     | WebAssembly client for interacting with Solana programs                   |

## Why?

The roots of Solana development have always been about "eating glass"—building against the odds with grit and determination. Lately, however, the ecosystem has matured. Development has become easier with the introduction of powerful browser libraries, SDKs, and world-class documentation.

This project asks: what if we went back to our roots? It's a return to the ethos of embracing difficulty and pain to build something truly meaningful. This library is for those who know there are easier languages than Rust, but choose to persevere regardless.

The path will not be easy. Hiring may be difficult, error messages may be cryptic, and progress can be painstaking. But you will build with Rust, you will make meaningful progress, and you will love it.

### Crate Details

- **[`memory_wallet`](./crates/memory_wallet)**: A `wallet-standard` compliant in-memory wallet, ideal for testing and prototyping. It manages `Keypair`s directly in memory, allowing for seamless signing of transactions and messages without requiring user interaction.

  ```rust
  use solana_sdk::transaction::Transaction;
  use solana_sdk::signature::Keypair;
  use wasm_client_solana::{SolanaRpcClient, WasmRpcClient};
  use memory_wallet::MemoryWallet;

  // 1. Create a mock RPC client
  let client = SolanaRpcClient::new("https://api.devnet.solana.com");

  // 2. Create a keypair for the wallet
  let keypair = Keypair::new();

  // 3. Instantiate the in-memory wallet
  let mut wallet = MemoryWallet::new(client, &[keypair]);

  // 4. The wallet can now be used to sign transactions, etc.
  // let signed_transaction = wallet.sign_transaction(Transaction::new_with_payer(&[], Some(&wallet.pubkey()))).await?;
  ```

- **[`test_utils_insta`](./crates/test_utils_insta)**: Provides helper functions for creating redactions in `insta` snapshot tests. This is useful for redacting dynamic data like signatures or timestamps, ensuring that snapshots remain consistent across test runs.

  ```rust
  use insta::assert_debug_snapshot;
  use solana_sdk::signature::Signature;
  use test_utils_insta::create_insta_redaction;

  let signature = Signature::new_unique();

  assert_debug_snapshot!("my_snapshot", &signature, {
    "signature" => create_insta_redaction(signature, "SIGNATURE"),
  });
  ```

- **[`test_utils_keypairs`](./crates/test_utils_keypairs)**: A collection of pre-defined, constant `Keypair`s for use in testing. This avoids the need to generate new keypairs in every test and provides known addresses for setting up test scenarios.

  ```rust
  use test_utils_keypairs::{get_admin_keypair, get_wallet_keypair};

  let admin_keypair = get_admin_keypair();
  let wallet_pubkey = get_wallet_keypair().pubkey();

  println!("Admin pubkey: {}", admin_keypair.pubkey());
  println!("Wallet pubkey: {}", wallet_pubkey);
  ```

- **[`test_utils_solana`](./crates/test_utils_solana)**: A suite of utilities for Solana integration testing. It simplifies the process of setting up a `ProgramTest` environment, managing test validators, and creating test accounts, streamlining the entire testing workflow.

  ```rust
  use solana_program_test::ProgramTest;
  use test_utils_solana::ProgramTestExtension;

  let mut program_test = ProgramTest::default();
  program_test.add_program("my_program", my_program_id, None);

  // Easily add an account with a specific balance
  program_test.add_account_with_lamports(
      some_pubkey,
      1_000_000_000, // 1 SOL
  );

  // let (mut banks_client, _, _) = program_test.start().await;
  ```

- **[`test_utils_anchor`](./crates/test_utils_anchor)**: Extends `test_utils_solana` with specific helpers for testing Anchor programs. It provides utilities for serializing and deserializing Anchor accounts, making it easier to set up and verify state in your tests.

  ```rust
  use anchor_lang::{Account, AnchorSerialize};
  use solana_program_test::ProgramTest;
  use test_utils_anchor::ProgramTestAnchorExtension;

  #[derive(Account, AnchorSerialize)]
  pub struct MyAnchorAccount {
      pub data: u64,
  }

  let mut program_test = ProgramTest::default();
  let my_account_data = MyAnchorAccount { data: 42 };

  // Add a serialzed Anchor account directly to the test environment
  program_test.add_account_with_anchor(
      my_account_pubkey,
      my_program_id,
      my_account_data,
      false,
  );
  ```

- **[`wasm_client_anchor`](./crates/wasm_client_anchor)**: A WebAssembly-compatible client for interacting with Anchor programs from a web environment. It provides a type-safe interface for building instructions and making RPC calls to your on-chain programs.

  ```rust
  // This crate is primarily for building client-side applications in WASM.
  // The following is a conceptual example of how you might define a client.

  // In your program's client library:
  // use wasm_client_anchor::prelude::*;
  //
  // #[program]
  // pub mod my_program {
  //     use super::*;
  //     pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
  // }
  //
  // #[derive(Accounts)]
  // pub struct Initialize {}
  //
  // // This would generate a client to call `initialize` from your WASM app.
  ```

- **[`wasm_client_solana`](./crates/wasm_client_solana)**: A general-purpose WebAssembly client for the Solana RPC API. It allows you to fetch account information, send transactions, and interact with the Solana network directly from the browser or other WASM runtimes.

  ```rust
  use std::str::FromStr;

  use solana_sdk::pubkey::Pubkey;
  use wasm_client_solana::SolanaRpcClient;
  use wasm_client_solana::WasmRpcClient;

  async fn get_balance() {
  	let client = SolanaRpcClient::new("https://api.devnet.solana.com");
  	let pubkey = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

  	let balance = client.get_balance(&pubkey).await;
  	println!("Balance: {:?}", balance);
  }
  ```

## Contributing

[`devenv`](https://devenv.sh/) is used to provide a reproducible development environment for this project. Follow the [getting started instructions](https://devenv.sh/getting-started/).

To automatically load the environment you should [install direnv](https://devenv.sh/automatic-shell-activation/) and then load the `direnv`.

```bash
direnv allow .
```

At this point you should see the `nix` commands available in your terminal. Any changes made to the `.envrc` file will require you to run the above command again.

Run the following command to install required rust binaries and solana tooling locally so you don't need to worry about polluting your global namespace or versioning.

```bash
install:all
```

### Upgrading `devenv`

```bash
nix profile upgrade devenv
```

### Editor Setup

To setup recommended configuration for your favorite editor run the following commands.

```bash
setup:vscode # Setup vscode
```

```bash
setup:helix
```

## License

Unlicense, see the [LICENSE](./license) file.
