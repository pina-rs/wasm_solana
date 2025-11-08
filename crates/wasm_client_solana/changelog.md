# Changelog

## 0.10.0 (2025-11-08)

### Breaking Changes

#### `v3` solana toolchain

Upgrade the solana toolchain to version 3 and begin the process of moving away from `solana_sdk`.

### Features

- Remove all references to `solana-sdk` and `solana-program`

### Documentation

- Update readme versions

## [0.9.0](https://github.com/solapino/wasm_solana/compare/wasm_client_solana@v0.8.0...wasm_client_solana@v0.9.0) - 2025-10-03

### <!-- 0 -->🎉 Added

- remove shared testing runner (doesn't work in threaded envs)
- build working with similar functionality
- add forks and run `cargo build -p wasm_solana_client`

### <!-- 3 -->📚 Documentation

- updates readmes

### <!-- 6 -->🧪 Testing

- make it work for wasm

### <!-- 7 -->⚙️ Miscellaneous Tasks

- reduce installation times
- update dependencies and improve token extension handling

### <!-- 9 -->◀️ Revert

- "fix: rollaback to solana-program `=2.1`"

## [0.8.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.5...wasm_client_solana@v0.8.0) - 2025-03-14

### <!-- 0 -->🎉 Added

- update anchor dependencies and improve configuration

### <!-- 1 -->🐛 Bug Fixes

- improve HTTP and WebSocket provider implementations
- rollaback to solana-program `=2.1`

### <!-- 5 -->🎨 Styling

- update formatting

## [0.7.5](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.4...wasm_client_solana@v0.7.5) - 2025-01-17

### <!-- 0 -->🎉 Added

- add `Clone` and `Copy` to more structs and enums
- implement `Clone` for token structs and enums

## [0.7.4](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.3...wasm_client_solana@v0.7.4) - 2025-01-16

### <!-- 1 -->🐛 Bug Fixes

- remove useless `SendWrapper` from `WebSocketProvider`

### <!-- 2 -->🚜 Refactor

- simplify `*_token_balance` handling in `UiTransactionStatusMeta`

## [0.7.3](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.2...wasm_client_solana@v0.7.3) - 2024-12-21

### <!-- 7 -->⚙️ Miscellaneous Tasks

- update Cargo.lock dependencies

## [0.7.2](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.1...wasm_client_solana@v0.7.2) - 2024-12-13

### <!-- 1 -->🐛 Bug Fixes

- make dependency more flexible
- `async-tungstenite@0.28.1` `futures-03-sink` feature

### <!-- 9 -->◀️ Revert

- "fix: make dependency more flexible"

## [0.7.1](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.7.0...wasm_client_solana@v0.7.1) - 2024-12-13

### <!-- 0 -->🎉 Added

- add `test_utils_keypairs` crate
- replace `test_utils` with `test_utils_insta`

## [0.7.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.6.1...wasm_client_solana@v0.7.0) - 2024-12-12

### <!-- 0 -->🎉 Added

- [**breaking**] add test_utils_anchor crate for testing anchor programs in wasm environments; update dependencies and configurations
- [**breaking**] upgrade to solana@v2 (#20)

### <!-- 7 -->⚙️ Miscellaneous Tasks

- update dependencies and configurations across multiple crates

## [0.6.1](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.6.0...wasm_client_solana@v0.6.1) - 2024-11-04

### <!-- 7 -->⚙️ Miscellaneous Tasks

- remove `unused import`

## [0.6.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.5.0...wasm_client_solana@v0.6.0) - 2024-10-20

### <!-- 0 -->🎉 Added

- [**breaking**] `SolanaRpcClient::get_nonce_account_*` methods

## [0.5.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.4.0...wasm_client_solana@v0.5.0) - 2024-10-13

### <!-- 0 -->🎉 Added

- [**breaking**] rename and add `authority` to `initialize_address_lookup_table`

### <!-- 1 -->🐛 Bug Fixes

- update instances of `pubkey` and `sign_message` after rename

## [0.4.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.6...wasm_client_solana@v0.4.0) - 2024-10-12

### <!-- 0 -->🎉 Added

- [**breaking**] add `async` methods directly to `VersionedTransactionExtension`
- add serialize and deserialize support to more method structs

## [0.3.6](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.5...wasm_client_solana@v0.3.6) - 2024-10-11

### <!-- 0 -->🎉 Added

- add `ToUiAccount` for converting from a `ReadableAccount` to a `UiAccount`
- support deserialize for `GetBalanceRequest`

## [0.3.5](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.4...wasm_client_solana@v0.3.5) - 2024-10-09

### <!-- 0 -->🎉 Added

- support deserialize `GetSignatureStatusesRequest`

## [0.3.4](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.3...wasm_client_solana@v0.3.4) - 2024-10-08

### <!-- 0 -->🎉 Added

- add `new_with_provider` method to `SolanaRpcClient`
- support deserialize `SimulateTransactionRequest`
- support deserialize `SendTransactionRequest`
- support deserialize `GetLatestBlockhashRequest`
- support `RpcProvider` in wasm environment
- support `ProgramTestContext` `RpcProvider`
- add `RpcProvider` trait

### <!-- 3 -->📚 Documentation

- add comment to `simulate_transaction` method

## [0.3.3](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.2...wasm_client_solana@v0.3.3) - 2024-10-07

### <!-- 0 -->🎉 Added

- implement `From<&SolanaRpcClient>` for `SolanaRpcClient`

## [0.3.2](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.1...wasm_client_solana@v0.3.2) - 2024-10-03

### <!-- 0 -->🎉 Added

- new `Unsubscription` struct which unsubscribes from rpc websocket methods

### <!-- 7 -->⚙️ Miscellaneous Tasks

- update formatting

## [0.3.1](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.3.0...wasm_client_solana@v0.3.1) - 2024-09-28

### <!-- 0 -->🎉 Added

- support browsers for subscriptions
- automate `AbortController::abort` on wasm `Request` drop

### <!-- 1 -->🐛 Bug Fixes

- websocket url generator
- wasm `HttpProvider` only aborts if fetch is still pending

### <!-- 6 -->🧪 Testing

- make wasm tests work in browser
- initial basic wasm tests

### <!-- 7 -->⚙️ Miscellaneous Tasks

- remove unused deps

## [0.3.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.2.1...wasm_client_solana@v0.3.0) - 2024-09-21

### <!-- 0 -->🎉 Added

- make `Context::slot` public
- make `spawn_local` public
- [**breaking**] remove `&mut` requirement for all `*_subscribe` methods

### <!-- 1 -->🐛 Bug Fixes

- update builder methods for `LogsSubscribeRequest`
- resolve broken support for streams

## [0.2.1](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.2.0...wasm_client_solana@v0.2.1) - 2024-09-18

### <!-- 3 -->📚 Documentation

- include crate `readme.md`

## [0.2.0](https://github.com/ifiokjr/wasm_solana/compare/wasm_client_solana@v0.1.0...wasm_client_solana@v0.2.0) - 2024-09-16

### <!-- 0 -->🎉 Added

- use native `Pubkey`, `Hash` and `Signature` types in structs

## [0.1.0](https://github.com/ifiokjr/wasm_solana/releases/tag/wasm_client_solana@v0.1.0) - 2024-09-12

### <!-- 0 -->🎉 Added

- use `WalletSolana` for signing anchor transactions
- add memory based standard wallet implementation
- add `Stream` support for solana client websockets
- add more websocket methods
- initial implementation of websockets
- initial commit

### <!-- 2 -->🚜 Refactor

- switch methods to use builder pattern

### <!-- 3 -->📚 Documentation

- prepare for initial release

### <!-- 5 -->🎨 Styling

- update lints
- update linting

### <!-- 6 -->🧪 Testing

- basic tests for `MemoryWallet` now succeed
- passing tests for `wasm_client_solana`
- write first tests
