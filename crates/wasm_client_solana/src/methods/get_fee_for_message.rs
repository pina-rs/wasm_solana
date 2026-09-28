use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_message::VersionedMessage;
use solana_message::v0;
use solana_message::v1;

use super::Context;
use crate::impl_http_method;

/// A message that can be priced by `getFeeForMessage`.
///
/// Every version serializes with the wire format rather than a serde derived
/// format, so the bytes are identical to what the cluster would receive in a
/// transaction. Legacy and v0 messages carry the address lookup table lookups
/// that the node needs to resolve; v1 messages carry their compute budget in
/// the message itself rather than in `ComputeBudget` instructions.
pub trait SerializableMessage {
	fn serialize(&self) -> Vec<u8>;
}

impl SerializableMessage for solana_message::Message {
	fn serialize(&self) -> Vec<u8> {
		self.serialize()
	}
}

impl SerializableMessage for v0::Message {
	fn serialize(&self) -> Vec<u8> {
		self.serialize()
	}
}

impl SerializableMessage for v1::Message {
	fn serialize(&self) -> Vec<u8> {
		self.serialize()
	}
}

impl SerializableMessage for VersionedMessage {
	fn serialize(&self) -> Vec<u8> {
		self.serialize()
	}
}

#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetFeeForMessageRequest {
	#[serde(serialize_with = "ser_message")]
	pub message: Vec<u8>,
	pub config: Option<CommitmentConfig>,
}

impl GetFeeForMessageRequest {
	pub fn new(message: &impl SerializableMessage) -> Self {
		Self {
			message: message.serialize(),
			config: None,
		}
	}

	pub fn new_with_config(message: &impl SerializableMessage, config: CommitmentConfig) -> Self {
		Self {
			message: message.serialize(),
			config: Some(config),
		}
	}
}

fn ser_message<S: serde::Serializer>(message: &[u8], ser: S) -> Result<S::Ok, S::Error> {
	ser.serialize_str(&BASE64_STANDARD.encode(message))
}

impl_http_method!(GetFeeForMessageRequest, "getFeeForMessage");

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct FeeForMessageValue(Option<u64>);

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetFeeForMessageResponse {
	pub context: Context,
	pub value: FeeForMessageValue,
}

impl From<GetFeeForMessageResponse> for u64 {
	fn from(val: GetFeeForMessageResponse) -> Self {
		val.value.0.unwrap_or_default()
	}
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use base64::prelude::BASE64_STANDARD;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	/// A base64 encoded legacy message, as returned by the cluster.
	const LEGACY_MESSAGE: &str = "AQABAgIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEBAQAA";

	fn legacy_message() -> solana_message::Message {
		wincode::deserialize(&BASE64_STANDARD.decode(LEGACY_MESSAGE).unwrap()).unwrap()
	}

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetFeeForMessageRequest::NAME)
			.id(1)
			.params(GetFeeForMessageRequest::new_with_config(
				&legacy_message(),
				CommitmentConfig::processed(),
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getFeeForMessage",
    "params": [
      "AQABAgIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEBAQAA",
      {
        "commitment": "processed"
      }
    ]
  }
  "###);
	}

	/// A v1 message embeds its compute budget, so pricing one never needs a
	/// `ComputeBudget` instruction and must still reach the wire intact.
	#[test]
	fn request_with_v1_versioned_message() {
		let message = v1::Message::new(
			solana_message::MessageHeader {
				num_required_signatures: 1,
				num_readonly_signed_accounts: 0,
				num_readonly_unsigned_accounts: 1,
			},
			v1::TransactionConfig::empty().with_priority_fee(5_000),
			solana_hash::Hash::new_from_array([9; 32]),
			vec![solana_pubkey::Pubkey::new_unique()],
			vec![],
		);
		let serialized = SerializableMessage::serialize(&message);
		assert_eq!(serialized[0], v1::V1_PREFIX);

		let request = GetFeeForMessageRequest::new(&VersionedMessage::V1(message));

		check!(request.message == serialized);
	}

	#[test]
	fn response() {
		let raw_json =
			r#"{"jsonrpc":"2.0","result":{"context":{"slot":5068},"value":5000},"id":1}"#;

		let response: ClientResponse<GetFeeForMessageResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");

		check!(response.result.context.slot == 5068);
		check!(response.result.value.0 == Some(5000));
	}
}
