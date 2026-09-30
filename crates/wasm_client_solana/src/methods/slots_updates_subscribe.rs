use serde::Deserialize;
use serde::Serialize;
use solana_clock::Slot;

use crate::impl_websocket_method;
use crate::impl_websocket_notification;

/// Request for the `slotsUpdatesSubscribe` RPC method: receive every
/// lifecycle event of every slot the node touches, from the first shred
/// arriving to the slot becoming a root. Unlike
/// [`slotSubscribe`](crate::SolanaRpcClient::slot_subscribe), which reports
/// one event per processed slot, this stream exposes the full pipeline, so a
/// client can distinguish dead slots, frozen banks, and optimistic
/// confirmations as they happen.
///
/// The method takes no parameters and the payload shape is treated as
/// unstable upstream: validators and RPC providers may not expose it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct SlotsUpdatesSubscribeRequest;

impl_websocket_method!(SlotsUpdatesSubscribeRequest, "slotsUpdates");

/// One lifecycle event in the life of a slot, tagged by `type` on the wire.
///
/// The variants follow the node's own pipeline order: a slot typically
/// arrives as [`SlotUpdate::FirstShredReceived`], becomes a bank
/// ([`SlotUpdate::CreatedBank`]), freezes ([`SlotUpdate::Frozen`]) and
/// completes ([`SlotUpdate::Completed`]); along the way it may be
/// optimistically confirmed or die, and eventually it is rooted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SlotUpdate {
	/// The first shred for this slot reached the node.
	FirstShredReceived {
		/// The slot the shred belongs to.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
	},
	/// The node finished replaying every entry in the slot.
	Completed {
		/// The completed slot.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
	},
	/// The node built a bank for the slot on top of its parent.
	CreatedBank {
		/// The newly created slot.
		slot: Slot,
		/// The parent slot the bank builds on.
		parent: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
	},
	/// The bank for the slot was frozen; replay counters are attached.
	Frozen {
		/// The frozen slot.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
		/// Replay statistics for the frozen bank.
		stats: SlotTransactionStats,
	},
	/// The slot failed replay and will not progress further.
	Dead {
		/// The dead slot.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
		/// Why the slot died.
		err: String,
	},
	/// The slot was optimistically confirmed by a supermajority, before
	/// finality.
	OptimisticConfirmation {
		/// The confirmed slot.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
	},
	/// The slot became a root and can no longer be rolled back.
	Root {
		/// The rooted slot.
		slot: Slot,
		/// Wall-clock time in milliseconds since the Unix epoch.
		timestamp: u64,
	},
}

/// Replay counters attached to [`SlotUpdate::Frozen`] notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotTransactionStats {
	/// Number of entry batches replayed for the slot.
	pub num_transaction_entries: u64,
	/// Transactions that replayed successfully.
	pub num_successful_transactions: u64,
	/// Transactions that failed during replay.
	pub num_failed_transactions: u64,
	/// Largest number of transactions in any single entry.
	pub max_transactions_per_entry: u64,
}

impl_websocket_notification!(SlotUpdate, "slotsUpdates");

#[cfg(test)]
mod tests {

	use assert2::check;

	use super::*;
	use crate::ClientRequest;
	use crate::SubscriptionResponse;
	use crate::methods::WebSocketMethod;
	use crate::methods::WebSocketNotification;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(SlotsUpdatesSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(SlotsUpdatesSubscribeRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "slotsUpdatesSubscribe"}"###);
	}

	#[test]
	fn notification_root() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "root",
					"slot": 79,
					"timestamp": 1625405860
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.method == SlotUpdate::NOTIFICATION);
		check!(
			notification.params.result
				== SlotUpdate::Root {
					slot: 79,
					timestamp: 1_625_405_860,
				}
		);
	}

	#[test]
	fn notification_created_bank() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "createdBank",
					"slot": 80,
					"parent": 79,
					"timestamp": 1625405861
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::CreatedBank {
					slot: 80,
					parent: 79,
					timestamp: 1_625_405_861,
				}
		);
	}

	#[test]
	fn notification_frozen() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "frozen",
					"slot": 80,
					"timestamp": 1625405862,
					"stats": {
						"numTransactionEntries": 3,
						"numSuccessfulTransactions": 96,
						"numFailedTransactions": 4,
						"maxTransactionsPerEntry": 32
					}
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::Frozen {
					slot: 80,
					timestamp: 1_625_405_862,
					stats: SlotTransactionStats {
						num_transaction_entries: 3,
						num_successful_transactions: 96,
						num_failed_transactions: 4,
						max_transactions_per_entry: 32,
					},
				}
		);
	}

	#[test]
	fn notification_dead() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "dead",
					"slot": 81,
					"timestamp": 1625405863,
					"err": "slot was skipped"
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::Dead {
					slot: 81,
					timestamp: 1_625_405_863,
					err: "slot was skipped".to_owned(),
				}
		);
	}

	#[test]
	fn notification_first_shred_received() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "firstShredReceived",
					"slot": 82,
					"timestamp": 1625405864
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::FirstShredReceived {
					slot: 82,
					timestamp: 1_625_405_864,
				}
		);
	}

	#[test]
	fn notification_optimistic_confirmation() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "optimisticConfirmation",
					"slot": 83,
					"timestamp": 1625405865
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::OptimisticConfirmation {
					slot: 83,
					timestamp: 1_625_405_865,
				}
		);
	}

	#[test]
	fn notification_completed() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotsUpdatesNotification",
			"params": {
				"result": {
					"type": "completed",
					"slot": 84,
					"timestamp": 1625405866
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotUpdate> =
			serde_json::from_str(raw_json).unwrap();

		check!(
			notification.params.result
				== SlotUpdate::Completed {
					slot: 84,
					timestamp: 1_625_405_866,
				}
		);
	}

	#[test]
	fn round_trip_serialization() {
		let update = SlotUpdate::Dead {
			slot: 90,
			timestamp: 1_625_405_900,
			err: "invalid Merkle proof".to_owned(),
		};

		let json = serde_json::to_value(&update).unwrap();

		insta::assert_compact_json_snapshot!(json, @r###"{"err": "invalid Merkle proof", "slot": 90, "timestamp": 1625405900, "type": "dead"}"###);
		check!(serde_json::from_value::<SlotUpdate>(json).unwrap() == update);
	}
}
