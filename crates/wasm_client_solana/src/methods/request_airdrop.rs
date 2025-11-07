use derive_more::derive::From;
use derive_more::derive::Into;
use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;

use crate::impl_http_method;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct RequestAirdropRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub lamports: u64,
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(RequestAirdropRequest, "requestAirdrop");

impl RequestAirdropRequest {
	pub fn new(pubkey: Pubkey, lamports: u64) -> Self {
		Self {
			pubkey,
			lamports,
			config: None,
		}
	}

	pub fn new_with_config(pubkey: Pubkey, lamports: u64, config: CommitmentConfig) -> Self {
		Self {
			pubkey,
			lamports,
			config: Some(config),
		}
	}
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, From, Into)]
pub struct RequestAirdropResponse(#[serde_as(as = "DisplayFromStr")] pub Signature);

#[cfg(test)]
mod tests {
	use std::str::FromStr;

	use assert2::check;
	use solana_sdk::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(RequestAirdropRequest::NAME)
			.id(1)
			.params(RequestAirdropRequest::new(
				pubkey!("83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri"),
				1_000_000_000,
			))
			.build();
		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "requestAirdrop",
    "params": [
      "83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri",
      1000000000
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":"5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW","id":1}"#;

		let response: ClientResponse<RequestAirdropResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 ==Signature::from_str("5VERv8NMvzbJMEkV8xnrLkEaWRtSz9CosKDYjCJjBRnbJLgp8uirBgmQpjKhoR4tjF3ZpRzrFmBV6UjKdiSZkQUW").unwrap());
	}
}
