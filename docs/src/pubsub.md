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

Dropping the last handle unsubscribes: a fire-and-forget request is sent so the node stops delivering frames. Call `subscription.unsubscribe().await` (or run the handle from `get_unsubscription()`) when the acknowledgement matters.

Each subscription has its own bounded channel (`SUBSCRIPTION_CHANNEL_CAPACITY` frames). A consumer that cannot keep up lags — the stream keeps going and the skipped count is available on `missed_notifications()` — instead of blocking the socket's other subscribers or growing without bound.

## The provider split

`WebSocketProvider` has two implementations selected by feature flags:

- **`js`** — the browser `WebSocket` API through web-sys; notifications arrive as `web_sys::MessageEvent` and are converted with the `ToWebSocketValue` trait.
- **`ssr`** — `reqwest-websocket` over tokio, for native binaries that still want pubsub.

Both speak the same JSON-RPC notification protocol, so app code is transport agnostic. One reader task per socket routes frames to the subscription they belong to; clones of a `Subscription` join at the channel's live edge.

## Streams utilities

`test_utils_solana` provides higher-level stream helpers for integration tests against a live validator:

```rust,ignore
use test_utils_solana::account_stream_subscription;
use test_utils_solana::log_stream_subscription;
```

- `log_stream_subscription` — subscribe to program logs and collect them until a condition matches (used to assert on-chain events).
- `account_stream_subscription` — watch account updates until a predicate fires.

These helpers make pubsub deterministic in tests: await the event instead of sleeping. `TestValidatorRunnerProps::enable_vote_subscription` opts a test validator into `voteSubscribe`, mirroring the production flag.

## Timeouts

Wasm browser sockets have no tokio to police them; `WASM_BINDGEN_TEST_TIMEOUT` governs test runs and the subscription streams surface connection failures as `ClientWebSocketError`. For native targets the provider relies on reqwest's own timeouts.
