use serde::Deserialize;
use serde::Serialize;
use solana_clock::Slot;
use typed_builder::TypedBuilder;

use crate::impl_websocket_method;
use crate::impl_websocket_notification;
use crate::rpc_config::RpcRootSubscribeConfig;

/// Request for the `rootSubscribe` RPC method: receive a notification every
/// time the node establishes a new root — a slot that can no longer be
/// rolled back. Fewer, safer events than
/// [`slotSubscribe`](crate::SolanaRpcClient::slot_subscribe): useful for
/// checkpointing UI state that must never show rolled-back data.
#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct RootSubscribeRequest {
	/// Commitment level for the reported roots.
	#[builder(default, setter(into))]
	pub config: RpcRootSubscribeConfig,
}

impl_websocket_method!(RootSubscribeRequest, "root");

impl Serialize for RootSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		#[derive(Serialize)]
		#[serde(rename = "RootSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(&'serde_tuple_inner RpcRootSubscribeConfig);

		let inner = Inner(&self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

/// Response for the `rootSubscribe` notification: the newly rooted slot,
/// delivered as a bare integer on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct RootNotificationResponse(pub Slot);

impl_websocket_notification!(RootNotificationResponse, "root");

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
			.method(RootSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(RootSubscribeRequest::builder().build())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "rootSubscribe", "params": [{}]}"###);
	}

	#[test]
	fn notification() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "rootNotification",
			"params": {
				"result": 79,
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<RootNotificationResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.method == RootNotificationResponse::NOTIFICATION);
		check!(notification.params.result.0 == 79);
	}
}
