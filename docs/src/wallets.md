# Wallets

Signing is modeled through the [Wallet Standard](https://github.com/pina-rs/wallet_standard), the wasm-native Rust implementation used across this ecosystem. `wasm_client_solana` does not sign anything itself; it bridges to wallet implementations through traits.

## The bridge

`wasm_client_solana::extensions` adds wallet-aware methods to its transaction and message types:

```rust,ignore
// sign a transaction with any wallet-standard Solana wallet
let signed = unsigned_transaction.sign_with_wallet(&wallet, options).await?;

// or sign and broadcast in one step through the wallet-standard capability
let props = SolanaSignAndSendTransactionProps::builder()
    .transaction(unsigned_transaction)
    .build();
let signature = wallet.sign_and_send_transaction(props).await?;
```

`W: WalletSolanaSignTransaction` and `W: WalletSolanaSignAndSendTransaction` bounds mean any compliant wallet works — `memory_wallet` in tests, an injected browser wallet through `wallet_standard_browser` in production.

## `memory_wallet`

The workspace ships a complete in-memory wallet for tests:

```rust,ignore
use memory_wallet::MemoryWallet;
use wasm_client_solana::SolanaRpcClient;
use test_utils_keypairs::get_wallet_keypair;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let mut wallet = MemoryWallet::new(client, &[get_wallet_keypair()]);
```

- Signs messages (`sign_message_async`) and transactions (`sign_transaction_async`).
- `sign_and_send_transaction_async` submits through the wallet's own `SolanaRpcClient`.
- Holds multiple accounts and exposes the Wallet Standard account/feature model.

Because it lives entirely in memory, tests get deterministic signing without a browser extension or keyring.

## Testing flows end to end

A typical integration test pairs `memory_wallet` with `test_utils_solana`'s validator runner:

1. Boot a validator (`TestValidatorRunner`) or a `ProgramTest` environment.
2. Point `SolanaRpcClient` at its RPC/websocket URLs.
3. Build a `MemoryWallet` over the client and the fixed keypairs from `test_utils_keypairs`.
4. Drive the wallet-standard signing traits and assert on chain state or stream events.

See [Testing](./testing.md) for the full recipe.
