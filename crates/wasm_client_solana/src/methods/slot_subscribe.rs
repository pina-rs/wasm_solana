use serde::Deserialize;
use serde::Serialize;
use solana_clock::Slot;
use typed_builder::TypedBuilder;

use crate::impl_websocket_method;
use crate::impl_websocket_notification;
use crate::rpc_config::RpcSlotSubscribeConfig;

/// Request for the `slotSubscribe` RPC method: receive a notification for
/// every slot the node processes, on the node's own schedule — the cheapest
/// way to drive a slot-based UI clock or detect a stalled connection.
#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct SlotSubscribeRequest {
	/// Commitment level for the reported slots.
	#[builder(default, setter(into))]
	pub config: RpcSlotSubscribeConfig,
}

impl_websocket_method!(SlotSubscribeRequest, "slot");

impl Serialize for SlotSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		// A config left at its default serializes as `null`, which the
		// request envelope then omits entirely: the node accepts
		// `slotSubscribe` with no parameters at all and rejects a
		// present-but-empty config object.
		let config = (self.config != RpcSlotSubscribeConfig::default()).then_some(&self.config);

		#[derive(Serialize)]
		#[serde(rename = "SlotSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(Option<&'serde_tuple_inner RpcSlotSubscribeConfig>);

		let inner = Inner(config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

/// The payload of a `slotNotification`: the slot the node just processed,
/// its parent (linking it into the fork), and — on node versions that report
/// it — when the node first saw the slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
	/// The parent slot, or 0 for the first slot of the ledger.
	pub parent: Slot,
	/// The slot the node just processed.
	pub slot: Slot,
	/// Wall-clock time the node first saw the slot, milliseconds since the
	/// Unix epoch. Agave omits this member, so it is `None` from most nodes.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub timestamp: Option<u64>,
}

impl_websocket_notification!(SlotInfo, "slot");

#[cfg(test)]
mod tests {

	use assert2::check;
	use solana_commitment_config::CommitmentConfig;

	use super::*;
	use crate::ClientRequest;
	use crate::SubscriptionResponse;
	use crate::methods::WebSocketMethod;
	use crate::methods::WebSocketNotification;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(SlotSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(
				SlotSubscribeRequest::builder()
					.config(RpcSlotSubscribeConfig {
						commitment: Some(CommitmentConfig::confirmed()),
					})
					.build(),
			)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "slotSubscribe", "params": [{"commitment": {"commitment": "confirmed"}}]}"###);
	}

	/// A default config serializes as `null`, which the request envelope then
	/// omits: the node rejects a present-but-empty config object.
	#[test]
	fn request_without_config_omits_params() {
		let request = ClientRequest::builder()
			.method(SlotSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(SlotSubscribeRequest::builder().build())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "slotSubscribe"}"###);
	}

	#[test]
	fn notification() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotNotification",
			"params": {
				"result": {
					"parent": 78,
					"slot": 79,
					"timestamp": 1625405805
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotInfo> = serde_json::from_str(raw_json).unwrap();

		check!(notification.method == SlotInfo::NOTIFICATION);
		check!(notification.params.result.slot == 79);
		check!(notification.params.result.parent == 78);
		check!(notification.params.result.timestamp == Some(1_625_405_805));
	}

	/// Agave nodes do not send a `timestamp` member at all; the notification
	/// must still parse or every slot notification is silently dropped.
	#[test]
	fn notification_without_timestamp() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "slotNotification",
			"params": {
				"result": {
					"parent": 78,
					"slot": 79,
					"root": 42
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<SlotInfo> = serde_json::from_str(raw_json).unwrap();

		check!(notification.params.result.slot == 79);
		check!(notification.params.result.timestamp.is_none());
	}
}
