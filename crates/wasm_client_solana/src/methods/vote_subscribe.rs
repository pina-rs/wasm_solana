use serde::Deserialize;
use serde::Serialize;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_clock::Slot;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::impl_websocket_method;
use crate::impl_websocket_notification;

/// Request for the `voteSubscribe` RPC method: receive every vote the node
/// observes, with the voting validator, the slots voted on, and the bank hash
/// it locked in. This is the raw consensus feed behind
/// [`get_vote_accounts`](crate::SolanaRpcClient::get_vote_accounts) — useful
/// for dashboards, but far noisier than
/// [`root_subscribe`](crate::SolanaRpcClient::root_subscribe).
///
/// This subscription is disabled by default; it can be enabled by passing
/// `--rpc-pubsub-enable-vote-subscription` to `solana-validator`.
///
/// The method takes no parameters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct VoteSubscribeRequest;

impl_websocket_method!(VoteSubscribeRequest, "vote");

/// The payload of a `voteNotification`: a single vote as gossiped by a
/// validator.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteNotification {
	/// The vote account that cast the vote.
	#[serde_as(as = "DisplayFromStr")]
	pub vote_pubkey: Pubkey,
	/// The slots the vote covers, in ascending order.
	pub slots: Vec<Slot>,
	/// The bank hash the validator locked in for the highest voted slot.
	#[serde_as(as = "DisplayFromStr")]
	pub hash: Hash,
	/// Wall-clock time the vote was generated, in seconds since the Unix
	/// epoch; validators may omit it for votes built without a timestamp
	/// instruction.
	pub timestamp: Option<i64>,
	/// The vote transaction signature.
	#[serde_as(as = "DisplayFromStr")]
	pub signature: Signature,
}

impl_websocket_notification!(VoteNotification, "vote");

#[cfg(test)]
mod tests {

	use assert2::check;
	use solana_hash::Hash;
	use solana_pubkey::Pubkey;
	use solana_signature::Signature;

	use super::*;
	use crate::ClientRequest;
	use crate::SubscriptionResponse;
	use crate::methods::WebSocketMethod;
	use crate::methods::WebSocketNotification;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(VoteSubscribeRequest::SUBSCRIBE)
			.id(1)
			.params(VoteSubscribeRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "voteSubscribe"}"###);
	}

	#[test]
	fn notification() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "voteNotification",
			"params": {
				"result": {
					"votePubkey": "6tHhbr7QTUT6Yug9L8GJZnsDVXh9U9LSMhDw3AYvJ9KR",
					"slots": [143, 144, 145],
					"hash": "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG",
					"timestamp": 1626060215,
					"signature": "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS"
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<VoteNotification> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.method == VoteNotification::NOTIFICATION);
		let result = notification.params.result;
		check!(
			result.vote_pubkey
				== "6tHhbr7QTUT6Yug9L8GJZnsDVXh9U9LSMhDw3AYvJ9KR"
					.parse::<Pubkey>()
					.unwrap()
		);
		check!(result.slots == [143, 144, 145]);
		check!(
			result.hash
				== "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG"
					.parse::<Hash>()
					.unwrap()
		);
		check!(result.timestamp == Some(1_626_060_215));
		check!(
			result.signature
				== "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS"
					.parse::<Signature>()
					.unwrap()
		);
	}

	#[test]
	fn notification_without_timestamp() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "voteNotification",
			"params": {
				"result": {
					"votePubkey": "6tHhbr7QTUT6Yug9L8GJZnsDVXh9U9LSMhDw3AYvJ9KR",
					"slots": [146],
					"hash": "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG",
					"signature": "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS"
				},
				"subscription": 0
			}
		}"#;

		let notification: SubscriptionResponse<VoteNotification> =
			serde_json::from_str(raw_json).unwrap();

		check!(notification.params.result.timestamp.is_none());
	}

	#[test]
	fn notification_rejects_malformed_pubkey() {
		let raw_json = r#"{
			"jsonrpc": "2.0",
			"method": "voteNotification",
			"params": {
				"result": {
					"votePubkey": "not-a-pubkey",
					"slots": [146],
					"hash": "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG",
					"signature": "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS"
				},
				"subscription": 0
			}
		}"#;

		let parsed: Result<SubscriptionResponse<VoteNotification>, _> =
			serde_json::from_str(raw_json);

		check!(parsed.is_err());
	}

	#[test]
	fn round_trip_serialization() {
		let vote = VoteNotification {
			vote_pubkey: "6tHhbr7QTUT6Yug9L8GJZnsDVXh9U9LSMhDw3AYvJ9KR"
				.parse()
				.unwrap(),
			slots: vec![147, 148],
			hash: "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG"
				.parse()
				.unwrap(),
			timestamp: None,
			signature: "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS"
				.parse()
				.unwrap(),
		};

		let json = serde_json::to_value(&vote).unwrap();

		insta::assert_compact_json_snapshot!(json, @r#"
		{
		  "hash": "8oYcQTzZ4qJBwsobeFsGavnVC5TcXMpMiaMpiix2X5uG",
		  "signature": "1GMkH3brNXiNNs1tiFZHu4yZSRrzJwxi5wB9bHFtMinfCXNnR1adh8Vo8NTheK4evneedH4qmvjeqcBBNAefgS",
		  "slots": [
		    147,
		    148
		  ],
		  "timestamp": null,
		  "votePubkey": "6tHhbr7QTUT6Yug9L8GJZnsDVXh9U9LSMhDw3AYvJ9KR"
		}
		"#);
		check!(serde_json::from_value::<VoteNotification>(json).unwrap() == vote);
	}
}
