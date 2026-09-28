---
wasm_client_solana: fix
---

# Keep websocket subscriptions alive after non-matching frames

Three websocket code paths returned `Poll::Pending` after consuming a frame that did not match what they were looking for: `Subscription::poll_next` (subscription acks replayed by the shared fork buffer, other subscriptions' notifications, undeserializable frames) and the `filter_map`-based ack waits in `create_subscription` and `Unsubscription::run` (the combinator returns `Pending` after a filtered item without re-polling).

`Forked` only registers the caller's waker with the underlying websocket once its buffer runs dry, so any `Pending` returned while buffered frames remain parked the consuming task with no wakeup armed — the first notification after a subscription ack was never delivered, and un-subscribing replayed history and stalled forever. The browser test for `accountSubscribe` has been silently failing for this reason (the CI wasm step is `continue-on-error`).

All three paths now re-poll in a loop until they find a matching frame, so the buffer drains and the socket waker is re-armed within the same poll. `poll_next` also stops rebuilding an `Unsubscription` on every poll. Verified in a real browser against both `solana-test-validator` and a local surfpool node, plus new Leptos/Dioxus Playwright examples under `examples/`.
