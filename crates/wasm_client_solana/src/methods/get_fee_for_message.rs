use serde::Deserialize;
use serde::Serializer;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_message::Message;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::serialize_and_encode;
use crate::solana_transaction_status::UiTransactionEncoding;

#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetFeeForMessageRequest {
	#[serde(serialize_with = "ser_message")]
	pub message: Message,
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetFeeForMessageRequest, "getFeeForMessage");

impl GetFeeForMessageRequest {
	pub fn new(message: Message) -> Self {
		Self {
			message,
			config: None,
		}
	}

	pub fn new_with_config(message: Message, config: CommitmentConfig) -> Self {
		Self {
			message,
			config: Some(config),
		}
	}
}

fn ser_message<S: Serializer>(msg: &Message, ser: S) -> Result<S::Ok, S::Error> {
	let message = serialize_and_encode::<Message>(msg, UiTransactionEncoding::Base64)
		.map_err(serde::ser::Error::custom)?;
	ser.serialize_str(&message)
}

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
	use base64::Engine;
	use base64::prelude::BASE64_STANDARD;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let decoded = BASE64_STANDARD.decode("AQABAgIAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEBAQAA").unwrap();
		let message = bincode::deserialize(&decoded).unwrap();
		let request = ClientRequest::builder()
			.method(GetFeeForMessageRequest::NAME)
			.id(1)
			.params(GetFeeForMessageRequest::new_with_config(
				message,
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
