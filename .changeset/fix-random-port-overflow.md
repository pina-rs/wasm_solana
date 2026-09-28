---
test_utils_solana: fix
---

# Fix a port-picker overflow near u16::MAX

`TestValidatorPorts::random_ports` drew its base port up to `u16::MAX - 25`, but the gossip range end is `port + 103`; a draw in the top ~80 values overflowed and panicked under debug overflow checks, failing test runs at random.
