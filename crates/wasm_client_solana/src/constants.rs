/// Compute unit ceiling a transaction may request, enforced by the runtime.
pub const COMPUTE_UNIT_MAX_LIMIT: usize = 1_400_000;
/// Compute unit limit applied when a caller does not estimate one.
pub const COMPUTE_UNIT_DEFAULT_LIMIT: usize = 200_000;
/// Addresses one `extend_lookup_table` instruction may add, which is the chunk
/// size used when populating a lookup table.
pub const MAX_LOOKUP_ADDRESSES_PER_TRANSACTION: usize = 30;

/// The maximum account data a transaction may load.
///
/// A v1 message must set this explicitly: the message default is zero, and a
/// transaction that loads any account data with a zero limit fails with
/// `MaxLoadedAccountsDataSizeExceeded`.
pub const MAX_LOADED_ACCOUNTS_DATA_SIZE_PER_TRANSACTION: usize = 64 * 1024 * 1024;

/// The highest transaction version this client can read.
///
/// The `txv1` feature gate is active on mainnet, so requests that omit
/// `maxSupportedTransactionVersion` can fail: a single v1 transaction makes the
/// node reject a whole `getBlock` and return `-32015` from `getTransaction`.
/// Reads therefore request v1 by default.
pub const MAX_SUPPORTED_TRANSACTION_VERSION: u8 = 1;
