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
		#[derive(Serialize)]
		#[serde(rename = "SlotSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(&'serde_tuple_inner RpcSlotSubscribeConfig);

		let inner = Inner(&self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

/// The payload of a `slotNotification`: the slot the node just processed,
/// its parent (linking it into the fork), and when the node saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotInfo {
	/// The parent slot, or 0 for the first slot of the ledger.
	pub parent: Slot,
	/// The slot the node just processed.
	pub slot: Slot,
	/// Wall-clock time the node first saw the slot, in milliseconds since the
	/// Unix epoch.
	pub timestamp: u64,
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
	}
}
