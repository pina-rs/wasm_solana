use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;

use super::Context;
use crate::impl_http_method;
use crate::rpc_response::RpcBlockhash;

/// Request for the `getLatestBlockhash` RPC method, which returns the most
/// recent blockhash and the last slot at which it remains valid.
#[skip_serializing_none]
#[derive(Debug, Default, Serialize_tuple, Deserialize_tuple)]
pub struct GetLatestBlockhashRequest {
	/// Commitment level for the request. Defaults to the client's commitment
	/// when omitted.
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetLatestBlockhashRequest, "getLatestBlockhash");

impl GetLatestBlockhashRequest {
	/// Creates a request that evaluates the latest blockhash using the node's
	/// default commitment.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a request that evaluates the latest blockhash at the given
	/// commitment level.
	pub fn new_with_config(config: CommitmentConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

/// Response for the `getLatestBlockhash` RPC method.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetLatestBlockhashResponse {
	/// The slot that the RPC node used to evaluate the request.
	pub context: Context,
	/// The blockhash and its last valid block height.
	pub value: RpcBlockhash,
}

#[cfg(test)]
mod tests {
	use assert2::check;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetLatestBlockhashRequest::NAME)
			.id(1)
			.params(GetLatestBlockhashRequest::new_with_config(
				CommitmentConfig::processed(),
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getLatestBlockhash", "params": [{"commitment": "processed"}]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":2792},"value":{"blockhash":"EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N","lastValidBlockHeight":3090}},"id":1}"#;
		let response: ClientResponse<GetLatestBlockhashResponse> =
			serde_json::from_str(raw_json).unwrap();
		let expected = ClientResponse {
			jsonrpc: String::from("2.0"),
			result: GetLatestBlockhashResponse {
				context: Context { slot: 2_792 },
				value: RpcBlockhash {
					blockhash: "EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N"
						.parse()
						.unwrap(),
					last_valid_block_height: 3_090,
				},
			},
			id: 1,
		};

		check!(response == expected);
	}
}
