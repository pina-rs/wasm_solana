use serde::Deserialize;
use serde::Serialize;
use serde_with::DisplayFromStr;
use solana_clock::Slot;
use solana_signature::Signature;
use solana_transaction_error::TransactionError;
use typed_builder::TypedBuilder;

use crate::Context;
use crate::impl_websocket_method;
use crate::impl_websocket_notification;
use crate::rpc_config::RpcSignatureSubscribeConfig;

/// Request for the `signatureSubscribe` RPC method: receive a notification
/// when a transaction with the given signature is confirmed (and optionally
/// when it is first received by the node).
///
/// This is the push-based alternative to polling
/// [`get_signature_statuses`](crate::SolanaRpcClient::get_signature_statuses)
/// in a loop, and the basis for confirming sends in one round trip.
#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct SignatureSubscribeRequest {
	/// The signature to watch.
	pub signature: Signature,
	/// Commitment level plus whether `receivedNotification` frames are
	/// enabled.
	#[builder(default, setter(into))]
	pub config: RpcSignatureSubscribeConfig,
}

impl_websocket_method!(SignatureSubscribeRequest, "signature");

impl Serialize for SignatureSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		#[derive(Serialize)]
		#[serde(rename = "SignatureSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(
			#[serde(with = "::serde_with::As::<DisplayFromStr>")] &'serde_tuple_inner Signature,
			&'serde_tuple_inner RpcSignatureSubscribeConfig,
		);

		let inner = Inner(&self.signature, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

/// The payload of a `signatureNotification`: the transaction's final status.
/// `err` is `None` when the transaction succeeded; the slot is where it was
/// processed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureResult {
	/// The transaction error, or `None` when the transaction succeeded.
	pub err: Option<TransactionError>,
	/// The slot in which the transaction was processed.
	pub slot: Slot,
}

/// Response for the `signatureSubscribe` notification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignatureNotificationResponse {
	/// Slot context for the notification.
	pub context: Context,
	/// The signature's final status.
	pub value: RpcSignatureResult,
}

impl_websocket_notification!(SignatureNotificationResponse, "signature");

/// Response for the `receivedNotification` frames enabled by
/// [`RpcSignatureSubscribeConfig::enable_received_notification`]: the node's
/// first sight of the transaction, before any confirmation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceivedNotificationResponse {
	/// Slot context for the notification.
	pub context: Context,
	/// The signature the node first received, base58-encoded.
	#[serde(with = "::serde_with::As::<DisplayFromStr>")]
	pub value: Signature,
}

#[cfg(test)]
mod tests {

	use assert2::check;
	use solana_commitment_config::CommitmentConfig;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::SubscriptionId;
	use crate::SubscriptionResult;
	use crate::methods::WebSocketMethod;
	use crate::methods::WebSocketNotification;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(SignatureSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(
				SignatureSubscribeRequest::builder()
					.signature(
						"5h6xBEauJ3PK6SWCZ1PGjBvj8vDdWG3KpwATGy1ARAXFSDwt8GFXM7W5Ncn16wmqokgpiKRLuS83KUxyZyv2sUYv"
							.parse()
							.unwrap(),
					)
					.config(RpcSignatureSubscribeConfig {
						commitment: Some(CommitmentConfig::finalized()),
						enable_received_notification: Some(true),
					})
					.build(),
			)
			.build();

		insta::assert_compact_json_snapshot!(request, @r#"
		{
		  "jsonrpc": "2.0",
		  "id": 1,
		  "method": "signatureSubscribe",
		  "params": [
		    "5h6xBEauJ3PK6SWCZ1PGjBvj8vDdWG3KpwATGy1ARAXFSDwt8GFXM7W5Ncn16wmqokgpiKRLuS83KUxyZyv2sUYv",
		    {
		      "commitment": "finalized",
		      "enableReceivedNotification": true
		    }
		  ]
		}
		"#);
	}

	#[test]
	fn subscription_result() {
		let raw_json = r#"{ "jsonrpc": "2.0", "result": 1027, "id": 1 }"#;

		let response: ClientResponse<SubscriptionId> = serde_json::from_str(raw_json).unwrap();
		let result: SubscriptionResult = response.into();

		check!(result.result == 1027);
	}

	#[test]
	fn notification() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "signatureNotification",
			"params": {
				"result": {
					"context": { "slot": 5207626 },
					"value": { "err": null, "slot": 5207624 }
				},
				"subscription": 1027
			}
		}"#;

		let notification: crate::SubscriptionResponse<SignatureNotificationResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.method == SignatureNotificationResponse::NOTIFICATION);
		check!(notification.params.subscription == 1027);
		check!(notification.params.result.value.err.is_none());
		check!(notification.params.result.value.slot == 5207624);
	}

	#[test]
	fn received_notification() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "receivedNotification",
			"params": {
				"result": {
					"context": { "slot": 5207626 },
					"value": "5h6xBEauJ3PK6SWCZ1PGjBvj8vDdWG3KpwATGy1ARAXFSDwt8GFXM7W5Ncn16wmqokgpiKRLuS83KUxyZyv2sUYv"
				},
				"subscription": 1027
			}
		}"#;

		let notification: crate::SubscriptionResponse<ReceivedNotificationResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.method == "receivedNotification");
	}
}
