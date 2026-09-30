#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/readme.md"))]

// The wire types come straight from the crates.io `-client-types` crates —
// upstream made them wasm-safe in 4.2.2 — so downstream code keeps the
// `wasm_client_solana::solana_*` paths it already used while gaining the
// upstream type identity shared with the rest of the ecosystem.
pub use solana_account_decoder_client_types;
pub use solana_account_decoder_wasm as solana_account_decoder;
pub use solana_transaction_status_client_types;
pub use solana_transaction_status_wasm as solana_transaction_status;

pub use crate::account_data::*;
pub use crate::client::*;
pub use crate::constants::*;
pub use crate::errors::*;
pub use crate::extensions::*;
pub use crate::methods::*;
pub use crate::providers::*;
pub use crate::rpc_config::*;
pub use crate::solana_client::*;
pub use crate::utils::spawn_local;

/// Hardened decoding of account data payloads received over RPC.
pub mod account_data;
mod client;
mod constants;
mod errors;
mod extensions;
mod methods;
/// Helpers for durable transaction nonce accounts.
pub mod nonce_utils;
mod providers;
/// Configuration types accepted by the RPC methods.
pub mod rpc_config;
/// Filter types used to narrow account and program queries.
pub mod rpc_filter;
/// Response types for the Solana JSON-RPC and websocket `PubSub` methods
/// exposed by this crate.
pub mod rpc_response;
/// Reward types attached to the reward list of a block.
pub mod runtime;
mod solana_client;
/// Utilities for running futures on the current runtime.
pub mod utils;

/// Re-exports of the helpers needed to drive subscriptions and sign
/// transactions.
pub mod prelude {
	/// `FutureExt`, `SinkExt`, `StreamExt` and friends for working with the
	/// futures returned by this crate.
	pub use futures::FutureExt;
	pub use futures::SinkExt;
	pub use futures::StreamExt;
	pub use futures::TryFutureExt;
	pub use futures::TryStreamExt;
	pub use wallet_standard::prelude::*;

	/// The transport trait implemented by the HTTP and websocket providers.
	pub use crate::RpcProvider;
	/// Conversion of a [`solana_message::VersionedMessage`] into a transaction.
	pub use crate::extensions::VersionedMessageExtension;
	/// Constructors and signing helpers for a
	/// [`VersionedTransaction`](solana_transaction::versioned::VersionedTransaction).
	pub use crate::extensions::VersionedTransactionExtension;
}
