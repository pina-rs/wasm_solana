use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_signature::Signature;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcSignatureStatusConfig;
use crate::solana_transaction_status::TransactionStatus;

#[derive(Clone, Debug)]
pub struct GetSignatureStatusesRequest {
	pub signatures: Vec<Signature>,
	pub config: Option<RpcSignatureStatusConfig>,
}

impl Serialize for GetSignatureStatusesRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[serde_as]
		#[skip_serializing_none]
		#[derive(Serialize)]
		#[serde(rename = "GetSignatureStatusesRequest")]
		struct Inner<'a>(
			#[serde_as(as = "Vec<DisplayFromStr>")] &'a Vec<Signature>,
			&'a Option<RpcSignatureStatusConfig>,
		);

		let inner = Inner(&self.signatures, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

impl<'de> Deserialize<'de> for GetSignatureStatusesRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[serde_as]
		#[skip_serializing_none]
		#[derive(Deserialize)]
		#[serde(rename = "GetSignatureStatusesRequest")]
		struct Inner(
			#[serde_as(as = "Vec<DisplayFromStr>")] Vec<Signature>,
			Option<RpcSignatureStatusConfig>,
		);

		let inner: Inner = Deserialize::deserialize(serde_tuple::Deserializer(deserializer))?;
		Ok(GetSignatureStatusesRequest {
			signatures: inner.0,
			config: inner.1,
		})
	}
}

impl_http_method!(GetSignatureStatusesRequest, "getSignatureStatuses");

impl GetSignatureStatusesRequest {
	pub fn new(signatures: Vec<Signature>) -> Self {
		Self {
			signatures,
			config: None,
		}
	}

	pub fn new_with_config(signatures: Vec<Signature>, config: RpcSignatureStatusConfig) -> Self {
		Self {
			signatures,
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetSignatureStatusesResponse {
	pub context: Context,
	pub value: Vec<Option<TransactionStatus>>,
}

#[cfg(test)]
mod tests {
	use std::str::FromStr;

	use assert2::check;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_transaction_status::TransactionConfirmationStatus;

	#[test]
	fn request() {
		let request = ClientRequest::builder().method(GetSignatureStatusesRequest::NAME)
				.id(1)
				.params(GetSignatureStatusesRequest::new_with_config(
					vec![Signature::from_str("5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW").unwrap()],
					RpcSignatureStatusConfig { search_transaction_history: true },
				))
				.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getSignatureStatuses",
    "params": [
      [
        "5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW"
      ],
      {
        "searchTransactionHistory": true
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":82},"value":[{"slot":48,"confirmations":null,"err":null,"status":{"Ok":null},"confirmationStatus":"finalized"},null]},"id":1}"#;

		let response: ClientResponse<GetSignatureStatusesResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 82);
		check!(
			response.result.value
				== vec![
					Some(TransactionStatus {
						slot: 48,
						err: None,
						status: Ok(()),
						confirmation_status: Some(TransactionConfirmationStatus::Finalized),
						confirmations: None
					}),
					None
				]
		);
	}
}
