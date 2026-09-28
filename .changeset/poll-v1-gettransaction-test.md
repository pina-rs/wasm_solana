---
memory_wallet: none
---

# Poll getTransaction in the v1 sign-and-send test

Test-only: the single-shot `getTransaction` read raced the completed-block store on slow CI runners (statuses reported the transaction confirmed before the block store could serve it), so the test has failed every CI run since the v1 support landed. It now polls for the same window the confirmation loop uses.
