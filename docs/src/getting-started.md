# Getting Started

## Installing

```toml
[dependencies]
wasm_client_solana = "0.12"
memory_wallet = "0.12" # optional: wallet-standard signing (great for tests)
```

### Feature flags of `wasm_client_solana`

| Feature          | Description                                                                        |
| ---------------- | ---------------------------------------------------------------------------------- |
| `js` _(wasm)_    | Browser transport: `fetch` for HTTP and the `WebSocket` API for pubsub. No tokio.  |
| `ssr` _(native)_ | Server transport: reqwest HTTP and reqwest-websocket, for desktop/server binaries. |
| `zstd`           | Native decoding for `base64+zstd` account data (pulls in the C zstd library).      |

```toml
# Browser (wasm32) target
wasm_client_solana = { version = "0.12", features = ["js"] }
# Native target
wasm_client_solana = { version = "0.12", features = ["ssr"] }
```

## getrandom on wasm32-unknown-unknown

`getrandom` 0.3+ needs an explicit backend on `wasm32-unknown-unknown`. This repository ships the wiring; copy it if your app generates randomness:

```toml
# .cargo/config.toml
[target.wasm32-unknown-unknown]
rustflags = ["--cfg", "getrandom_backend=\"wasm_js\""]
```

```toml
# crates/*/Cargo.toml
[target.'cfg(all(target_arch = "wasm32", target_os = "unknown"))'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
```

## Quickstart: read a balance

```rust,ignore
use wasm_client_solana::SolanaRpcClient;
use solana_pubkey::Pubkey;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let pubkey = "So11111111111111111111111111111111111111112".parse()?;

let lamports = client.get_balance(&pubkey).await?;
println!("lamports: {lamports}");
```

`SolanaRpcClient` is the one constructor for every environment — it selects the `HttpProvider` for `ssr` builds and the browser transport for wasm builds behind the same API.

## Quickstart: sign and send with a wallet

```rust,ignore
use memory_wallet::MemoryWallet;
use wasm_client_solana::SolanaRpcClient;
use test_utils_keypairs::get_wallet_keypair;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let mut wallet = MemoryWallet::new(client, &[get_wallet_keypair()]);

// Sign through the Wallet Standard traits:
let signed = wallet
    .sign_message_async(b"hello solana")
    .await?;
```

`MemoryWallet` also implements `WalletSolanaSignTransaction` and `WalletSolanaSignAndSendTransaction`, so transaction flows go through the same traits (see [Wallets](./wallets.md)).

## Quickstart: pubsub

```rust,ignore
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::prelude::*;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let subscription = client.account_subscribe(pubkey).await?;
let mut stream = subscription.take(10);

while let Some(notification) = stream.next().await {
    log::info!("account changed: {notification:?}");
}
// The last handle's drop unsubscribes.
```

See [Pubsub and Streams](./pubsub.md) for the full subscription model.

## Toolchain

The workspace pins the latest stable Rust in `rust-toolchain.toml` and targets `wasm32-unknown-unknown` + `wasm32-wasip1`. Building the wasm target:

```bash
cargo build -p wasm_client_solana --target wasm32-unknown-unknown -F js
```

For `wasm-bindgen` test runs the devenv environment provides `wasm-bindgen-cli`, chromedriver and a local validator — see [Testing](./testing.md).
