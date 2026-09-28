<!-- {@commitmentField} -->

Commitment level for the request. When omitted the field is left out of the payload, so the node applies its own default. The client's commitment is only applied by the convenience methods that build a config from it, such as `get_balance`.

<!-- {/commitmentField} -->

<!-- {@encodingAccount} -->

Encoding format for account data.

<!-- {/encodingAccount} -->

<!-- {@encodingTransaction} -->

Encoding format for transaction data. Use `base64` for anything that may exceed the 1232-byte base58 limit, such as a v1 transaction.

<!-- {/encodingTransaction} -->

<!-- {@minContextSlot} -->

The minimum slot that the request can be evaluated at. Enforced only when the commitment is `processed`.

<!-- {/minContextSlot} -->

<!-- {@maxSupportedTransactionVersion} -->

The highest transaction version to return. Defaults to `1` so that v1 transactions are readable; pass `None` to opt out.

<!-- {/maxSupportedTransactionVersion} -->

<!-- {@transactionDetails} -->

Level of transaction detail to return. `Full` includes transaction messages and metadata; `Accounts` returns only account keys; `None` omits transactions entirely.

<!-- {/transactionDetails} -->

<!-- {@txv1WireFormat} -->

Transaction bytes use the version dependent `wincode` wire format. Legacy and v0 transactions place a `short_vec` signature count first; v1 transactions place the message first behind a `0x81` discriminator and move the signatures to the tail.

<!-- {/txv1WireFormat} -->

<!-- {@txv1ComputeBudget} -->

A v1 message carries its compute budget instead of using `ComputeBudget` instructions, and defaults both `compute_unit_limit` and `loaded_accounts_data_size_limit` to zero. A transaction carrying zeros fails with `MaxLoadedAccountsDataSizeExceeded`, so set the limits explicitly.

<!-- {/txv1ComputeBudget} -->

<!-- {@txv1ReadHint} -->

The `txv1` feature gate is active on mainnet. A request that omits `maxSupportedTransactionVersion` fails when it meets a v1 transaction: `getTransaction` returns `-32015`, and a single v1 transaction rejects a whole `getBlock`.

<!-- {/txv1ReadHint} -->
