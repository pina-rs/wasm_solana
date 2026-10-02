---
wasm_client_solana: fix
---

# Deliver websocket subscriptions past non-matching frames

The shared-fork pubsub implementation had three paths that returned `Poll::Pending` after consuming a buffered frame that did not match — subscription ack waits, notification filtering, and unsubscription waits — while the underlying fork only armed the socket waker once its buffer ran dry, parking the consuming task forever. The browser test for `accountSubscribe` had been silently failing for exactly this reason (the CI wasm step is `continue-on-error`).

The immediate fix re-polled in a loop until a matching frame arrived; the released design replaces the shared fork with a per-subscription router (see "Route pubsub through per-subscription channels"), which removes the buffered-replay machinery the wakeup bug lived in altogether. Verified in a real browser against both `solana-test-validator` and a local surfpool node, plus the Leptos/Dioxus Playwright examples under `examples/`.
