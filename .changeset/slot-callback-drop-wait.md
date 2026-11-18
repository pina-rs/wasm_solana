---
core: none
---

# Wait out the slot-subscription handshake before asserting that drop stops callbacks

`dropping_the_handle_stops_the_callback` slept two seconds and then required a nonzero count. On a loaded runner the websocket handshake can take longer than that, so the assertion failed before the subscription had delivered anything. The test now waits up to the same 30 seconds as the ordered-slot callback test, then still checks that dropping the handle stops further callbacks.
