---
core: fix
---

# Correct acknowledgement benchmark measurements

Buffer replay history before timing, consume it before checking a uniquely identified JSON-RPC acknowledgement, and exclude fixture destruction from both subscription benchmark timings.

The benchmark workflow uses the head subscription harness for both compared revisions, so fixture corrections are not reported as runtime improvements or regressions.
