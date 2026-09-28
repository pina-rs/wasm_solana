//! Request and response types for the Solana JSON-RPC API.
//!
//! Each submodule models exactly one RPC method: a request type that serializes
//! to the JSON-RPC `params` array and a response type that deserializes the
//! `result` field. Requests implement [`HttpMethod`] to carry the method name,
//! and the websocket-backed ones additionally implement
//! [`WebSocketMethod`]/[`WebSocketNotification`] to carry the subscription
//! method names.

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use solana_clock::Slot;

pub use self::get_account_info::*;
pub use self::get_balance::*;
pub use self::get_block::*;
pub use self::get_block_commitment::*;
pub use self::get_block_height::*;
pub use self::get_block_production::*;
pub use self::get_block_time::*;
pub use self::get_blocks::*;
pub use self::get_blocks_with_limit::*;
pub use self::get_cluster_nodes::*;
pub use self::get_epoch_info::*;
pub use self::get_epoch_schedule::*;
pub use self::get_fee_for_message::*;
pub use self::get_first_available_block::*;
pub use self::get_genesis_hash::*;
pub use self::get_health::*;
pub use self::get_highest_snapshot_slot::*;
pub use self::get_identity::*;
pub use self::get_inflation_governor::*;
pub use self::get_inflation_rate::*;
pub use self::get_inflation_reward::*;
pub use self::get_largest_accounts::*;
pub use self::get_latest_blockhash::*;
pub use self::get_leader_schedule::*;
pub use self::get_max_retransmit_slot::*;
pub use self::get_minimum_balance_for_rent_exemption::*;
pub use self::get_multiple_accounts::*;
pub use self::get_program_accounts::*;
pub use self::get_recent_performance_samples::*;
pub use self::get_recent_prioritization_fees::*;
pub use self::get_signature_statuses::*;
pub use self::get_signatures_for_address::*;
pub use self::get_slot::*;
pub use self::get_slot_leader::*;
pub use self::get_slot_leaders::*;
pub use self::get_stake_activation::*;
pub use self::get_stake_minimum_delegation::*;
pub use self::get_supply::*;
pub use self::get_token_account_balance::*;
pub use self::get_token_accounts_by_delegate::*;
pub use self::get_token_accounts_by_owner::*;
pub use self::get_token_largest_accounts::*;
pub use self::get_token_supply::*;
pub use self::get_transaction::*;
pub use self::get_transaction_count::*;
pub use self::get_version::*;
pub use self::get_vote_accounts::*;
pub use self::is_blockhash_valid::*;
pub use self::minimum_ledger_slot::*;
pub use self::request_airdrop::*;
pub use self::send_transaction::*;
pub use self::simulate_transaction::*;

mod get_account_info;
mod get_balance;
mod get_block;
mod get_block_commitment;
mod get_block_height;
mod get_block_production;
mod get_block_time;
mod get_blocks;
mod get_blocks_with_limit;
mod get_cluster_nodes;
mod get_epoch_info;
mod get_epoch_schedule;
mod get_fee_for_message;
mod get_first_available_block;
mod get_genesis_hash;
mod get_health;
mod get_highest_snapshot_slot;
mod get_identity;
mod get_inflation_governor;
mod get_inflation_rate;
mod get_inflation_reward;
mod get_largest_accounts;
mod get_latest_blockhash;
mod get_leader_schedule;
mod get_max_retransmit_slot;
mod get_minimum_balance_for_rent_exemption;
mod get_multiple_accounts;
mod get_program_accounts;
mod get_recent_performance_samples;
mod get_recent_prioritization_fees;
mod get_signature_statuses;
mod get_signatures_for_address;
mod get_slot;
mod get_slot_leader;
mod get_slot_leaders;
mod get_stake_activation;
mod get_stake_minimum_delegation;
mod get_supply;
mod get_token_account_balance;
mod get_token_accounts_by_delegate;
mod get_token_accounts_by_owner;
mod get_token_largest_accounts;
mod get_token_supply;
mod get_transaction;
mod get_transaction_count;
mod get_version;
mod get_vote_accounts;
mod is_blockhash_valid;
mod minimum_ledger_slot;
mod request_airdrop;
mod send_transaction;
mod simulate_transaction;

/// Slot at which a response was evaluated, returned by every context-aware RPC
/// response.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Context {
	/// The slot that the RPC node used to evaluate the request.
	pub slot: Slot,
}

/// Inclusive range of slots used to scope block production queries, where
/// `first_slot` is the starting slot and `last_slot` bounds the range at the
/// tip of the ledger when omitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockProductionRange {
	/// The first slot in the range. Only blocks at or after this slot are
	/// returned.
	pub first_slot: u64,
	/// The last slot to consider. Defaults to the current tip of the ledger
	/// when omitted.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub last_slot: Option<u64>,
}

/// A request that maps to a JSON-RPC method on the HTTP endpoint.
pub trait HttpMethod: Serialize {
	/// Name of the JSON-RPC method, such as `getBalance`.
	const NAME: &'static str;
}

macro_rules! impl_http_method {
	($ident:ident, $name:literal) => {
		impl $crate::methods::HttpMethod for $ident {
			const NAME: &'static str = $name;
		}
	};
}

/// A websocket notification payload that deserializes from a subscription
/// notification.
pub trait WebSocketNotification: DeserializeOwned {
	/// Name of the notification, such as `accountNotification`.
	const NOTIFICATION: &'static str;
	/// Name of the method that cancels the subscription, such as
	/// `accountUnsubscribe`.
	const UNSUBSCRIBE: &'static str;
}

/// A request that maps to a subscription method on the websocket endpoint.
pub trait WebSocketMethod: Serialize {
	/// Name of the subscribe method, such as `accountSubscribe`.
	const SUBSCRIBE: &'static str;
}

macro_rules! impl_websocket_method {
	($ident:ident, $prefix:literal) => {
		impl $crate::methods::WebSocketMethod for $ident {
			const SUBSCRIBE: &'static str = concat!($prefix, "Subscribe");
		}
	};
}
macro_rules! impl_websocket_notification {
	($ident:ident, $prefix:literal) => {
		impl $crate::methods::WebSocketNotification for $ident {
			const NOTIFICATION: &'static str = concat!($prefix, "Notification");
			const UNSUBSCRIBE: &'static str = concat!($prefix, "Unsubscribe");
		}
	};
}

pub(crate) use impl_http_method;
pub(crate) use impl_websocket_method;
pub(crate) use impl_websocket_notification;
