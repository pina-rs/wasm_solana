---
wasm_client_solana: major
---

# Pubsub reliability: rejections, slot feed, fork wakeups

Breaking: `SlotInfo.timestamp` is now `Option<u64>`, so code reading the field must handle `None`.

Four defects in the pubsub path are fixed, three of them capable of hanging or silencing a healthy subscription:

- **Rejected subscriptions no longer hang.** When a node answered a `*Subscribe` or `*Unsubscribe` request with a JSON-RPC error envelope, the acknowledgement loop treated the reply as "not mine yet" and waited forever. Both loops now fail fast with `ClientWebSocketError::Subscription`/`Unsubscription` and log the node's reason, so an invalid filter or a permission-gated feed surfaces immediately instead of parking the caller.
- **`slotSubscribe` and `rootSubscribe` sent a config the node rejects.** A default config serialized as `[{}]`, which agave refuses ("No parameters were expected"); `logsSubscribe` sent `["allWithVotes",{}]`, refused by several releases with "missing field `commitment`". A config left at its default now serializes as `null` — which the request envelope then omits entirely for the no-parameter feeds — matching the wire shape every node accepts. `slotSubscribe` and `rootSubscribe` now actually work against agave; previously no test covered them and every call hung on the rejected handshake.
- **`SlotInfo.timestamp` is now `Option<u64>`.** Agave sends `{parent, slot, root}` with no `timestamp` member, so every `slotNotification` failed to deserialize and was silently dropped — a slot subscription delivered nothing, ever. The field is `None` from nodes that omit it.
- **Parked subscriptions can no longer starve behind dead wakers.** The pubsub fan-out now uses a vendored, corrected `fork_stream` (the published 0.1.0 remains a dependency of nothing here): its single source-registered waker meant the last task to poll the socket — say a confirmation's unsubscription — left its dying waker with the socket, and parked forks queued behind stale wakers never heard about later frames; an `accountSubscribe` alongside a `send_and_confirm` could silently stop delivering. The vendored fork registers every parked fork through one shared waker and wakes all of them when a frame lands, at the cost of an occasional spurious no-op wake. This is also why the `fork_stream` dependency is gone from the manifest.
