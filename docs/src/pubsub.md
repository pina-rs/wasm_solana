# Pubsub and Streams

## Subscriptions

The client exposes a typed subscribe method per pubsub feed. Each returns a `Subscription<T>` stream; the provider multiplexes all of them over a single websocket.

| Method                    | Yields                                 | Notes                                                                      |
| ------------------------- | -------------------------------------- | -------------------------------------------------------------------------- |
| `account_subscribe`       | account state each time it changes     | one of the two feeds every dApp needs                                      |
| `program_subscribe`       | accounts owned by a program            | the `getProgramAccounts` push equivalent                                   |
| `logs_subscribe`          | transaction logs                       | filter by program or include votes                                         |
| `signature_subscribe`     | final status of one transaction        | the push alternative to polling `getSignatureStatuses`                     |
| `slot_subscribe`          | every processed slot                   | the cheapest UI clock and stall detector                                   |
| `slots_updates_subscribe` | the full slot lifecycle (`SlotUpdate`) | shreds → bank → frozen → root; noisiest feed, not served by every provider |
| `root_subscribe`          | slots that can no longer roll back     | safe checkpoint signal                                                     |
| `vote_subscribe`          | raw votes as gossiped                  | requires `--rpc-pubsub-enable-vote-subscription` on the validator          |
| `block_subscribe`         | confirmed/finalized blocks             | requires `--rpc-pubsub-enable-block-subscription` on the validator         |

```rust,ignore
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::prelude::*;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let subscription = client.slots_updates_subscribe().await?;
let mut updates = subscription.take(4);

while let Some(update) = updates.next().await {
    // typed SlotUpdate values: FirstShredReceived, CreatedBank, Frozen, Root...
}
```

Dropping a `Subscription` stops its consumption but does **not** unsubscribe on the node: async work cannot run from `Drop`, so the server keeps delivering frames until the socket closes. Call `subscription.unsubscribe().await` — or stash the handle from `get_unsubscription()` and run it later — when the subscription should actually end.

Frames are matched to their subscription by notification method and subscription id _before_ any typed deserialization, so a subscription only pays the full parse for frames that belong to it. On a noisy socket (for example a `slotsUpdates` feed alongside an account subscription) each fork skips foreign frames with two cheap JSON pointer probes.

## Callback subscriptions

Every stream subscribe method has an `on_*` callback counterpart for code that prefers push over polling a stream:

| Method              | Callback receives                                |
| ------------------- | ------------------------------------------------ |
| `on_account_change` | the account payload of `account_subscribe`       |
| `on_program_change` | the keyed account payload of `program_subscribe` |
| `on_logs`           | the logs payload of `logs_subscribe`             |
| `on_signature`      | the final status of the watched signature        |
| `on_slot_change`    | each processed `SlotInfo`                        |
| `on_slots_updates`  | each `SlotUpdate` lifecycle event                |
| `on_root_change`    | each new root slot                               |
| `on_vote`           | each gossiped vote                               |
| `on_block`          | each confirmed/finalized block                   |

```rust,ignore
use wasm_client_solana::SlotSubscribeRequest;
use wasm_client_solana::SolanaRpcClient;

let client = SolanaRpcClient::new("https://api.devnet.solana.com");
let slots = client
    .on_slot_change(SlotSubscribeRequest::builder().build(), |slot| {
        log::info!("new slot {}", slot.slot);
    })
    .await?;
```

The crate drives the subscription and invokes the closure on the target's runtime — `wasm-bindgen-futures` in the browser, `tokio::spawn` under `ssr` (so the closure must be `Send` there; in the browser it may capture `Rc` or JavaScript handles freely). No spawned consumer task is needed on the caller's side.

The returned `CallbackSubscription` handle controls the pump's lifetime:

- **`unsubscribe().await`** stops the callback, sends the server unsubscribe and waits for its ack.
- **Dropping the handle** stops the callback and asks the server to unsubscribe in the background — the fire-and-forget teardown a stream's `Drop` cannot offer, because the pump task sends the request as it exits. Nothing waits for the ack.
- **`detach()`** keeps the pump running after the handle is dropped, for subscriptions that should live as long as the connection; no unsubscription is ever sent.
- **`is_stopped()`** reports whether the pump has ended, through either path above or a dead socket.

A generic `subscribe_with_callback(request, callback)` covers feeds without a dedicated wrapper. A node that rejects a subscription request fails the `on_*` call immediately with a `ClientWebSocketError` (the node's reason is logged) instead of waiting for an acknowledgement that will never come.

## The provider split

`WebSocketProvider` has two implementations selected by feature flags:

- **`js`** — the browser `WebSocket` API through gloo; notifications arrive as text or binary frames and are converted with the `ToWebSocketValue` trait.
- **`ssr`** — `reqwest-websocket` over tokio, for native binaries that still want pubsub.

Both speak the same JSON-RPC notification protocol, so app code is transport agnostic. There is no reader task routing frames: the socket's stream is split and fanned out through `fork_stream`, and each `Subscription` (and callback pump) reads its own fork of the shared buffer, positioned at the live edge when the subscription was created.

## Integration testing feeds

`test_utils_solana` runs pubsub tests against a real validator (`test_validator` feature) or an embedded surfpool node (`surfpool` feature):

```rust,ignore
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;

let runner = TestValidatorRunnerProps::builder()
    .pubkeys(vec![get_wallet_keypair().pubkey()])
    .build()
    .run()
    .await?;
let subscription = runner.rpc().logs_subscribe(/* ... */).await?;
```

The crate's own test suites (`tests/streams.rs`, `tests/callbacks.rs`, `tests/confirm.rs`) are worked examples of asserting on stream and callback deliveries; `TestValidatorRunnerProps::enable_vote_subscription` opts a test validator into `voteSubscribe`, mirroring the production flag.

## Timeouts

Wasm browser sockets have no tokio to police them; `WASM_BINDGEN_TEST_TIMEOUT` governs test runs and the subscription streams surface connection failures as `ClientWebSocketError`. For native targets the provider relies on reqwest's own timeouts.
