---
wasm_client_solana: breaking
---

# Route pubsub through per-subscription channels

The websocket provider no longer shares one fork-of-a-buffer between every subscription. A single reader task now inspects each incoming frame once, matches responses to the request waiting on their id, and routes notifications into a bounded per-subscription channel (`SUBSCRIPTION_CHANNEL_CAPACITY` = 1024 frames).

The old design kept a never-read root fork alive so new forks could attach, which meant the shared buffer could never advance: every frame the socket had ever received stayed in memory for the provider's lifetime — on the order of 100 MB/day of dead JSON for a dApp holding one account subscription — and one slow consumer stalled buffer advancement for every other subscriber. Memory is now proportional to live subscriptions, a stalled consumer only lags its own channel (observable through `Subscription::missed_notifications()`), and the reader retains nothing.

Dropping the last handle of a subscription now unsubscribes: a fire-and-forget request is sent on the shared socket so the node stops delivering frames (validators cap concurrent subscriptions; leaked ones previously kept streaming into the retained buffer forever). `unsubscribe()` and `Unsubscription::run()` still wait for the ack when confirmation matters. The fork machinery (`fork_stream`, the lost-wakeup re-poll loops, the live-edge weak handles) is gone entirely, together with the class of wakeup bugs it kept re-growing.

Breaking: `WebSocketProvider::create_subscription` and `Subscription::from_parts` hand back a `tokio::sync::broadcast::Receiver` instead of a forked stream, and `Unsubscription` no longer has a builder — construct it through `Subscription::get_unsubscription`. Clones of a `Subscription` join at the channel's live edge rather than duplicating a fork position.

Routing costs ~1–2 µs per notification at 1–64 registered subscriptions (see the `subscription/route_1000` benchmark), the same order as parsing the frame itself; subscription setup no longer touches socket history at all.
