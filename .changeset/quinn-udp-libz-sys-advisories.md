---
core: fix
---

# Patch quinn-udp and libz-sys advisories

Refresh the lockfile to quinn-udp 0.5.16 and libz-sys 1.1.33. RUSTSEC-2026-0538 is a receive-loop spin after a truncated datagram, and RUSTSEC-2026-0544 is a heap overflow in zlib-ng. Both versions stay inside the ranges already required by reqwest, quinn, and the validator stack. Public client APIs are unchanged.
