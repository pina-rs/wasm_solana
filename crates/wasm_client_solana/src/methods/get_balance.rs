use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;

/// Request for the `getBalance` RPC method, which returns the lamport balance
/// of an account at a given commitment.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct GetBalanceRequest {
	/// Base58 pubkey of the account to query.
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	/// Commitment level for the request. Defaults to the client's commitment
	/// when omitted.
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetBalanceRequest, "getBalance");

impl GetBalanceRequest {
	/// Creates a request that evaluates the balance at the client's default
	/// commitment.
	pub fn new(pubkey: Pubkey) -> Self {
		Self {
			pubkey,
			config: None,
		}
	}

	/// Creates a request that evaluates the balance at the given commitment
	/// level.
	pub fn new_with_config(pubkey: Pubkey, config: CommitmentConfig) -> Self {
		Self {
			pubkey,
			config: Some(config),
		}
	}
}

/// Response for the `getBalance` RPC method.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetBalanceResponse {
	/// The slot that the RPC node used to evaluate the request.
	pub context: Context,
	/// The account balance in lamports at the requested commitment.
	pub value: u64,
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_pubkey::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let pubkey = pubkey!("83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri");
		let request = ClientRequest::builder()
			.method(GetBalanceRequest::NAME)
			.id(1)
			.params(GetBalanceRequest::new(pubkey))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getBalance", "params": ["83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri"]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1},"value":0},"id":1}"#;

		let response: ClientResponse<GetBalanceResponse> = serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1);
		check!(response.result.value == 0);
	}
}
