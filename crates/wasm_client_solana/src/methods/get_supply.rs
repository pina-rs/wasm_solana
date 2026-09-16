use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcSupplyConfig;
use crate::rpc_response::RpcSupply;

/// Request for the `getSupply` RPC method, which returns the total, circulating
/// and non-circulating supply in lamports.
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Default)]
pub struct GetSupplyRequest {
	config: Option<RpcSupplyConfig>,
}

impl_http_method!(GetSupplyRequest, "getSupply");

impl GetSupplyRequest {
	/// Creates a request using the default supply config, which omits the
	/// non-circulating accounts list.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a request with an explicit supply config.
	pub fn new_with_config(config: RpcSupplyConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

/// Response for the `getSupply` RPC method.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetSupplyResponse {
	/// The slot that the RPC node used to evaluate the request.
	pub context: Context,
	/// Total, circulating, and non-circulating lamport supply.
	pub value: RpcSupply,
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
			.method(GetSupplyRequest::NAME)
			.id(1)
			.params(GetSupplyRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getSupply"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1114},"value":{"circulating":16000,"nonCirculating":1000000,"nonCirculatingAccounts":["FEy8pTbP5fEoqMV1GdTz83byuA8EKByqYat1PKDgVAq5","9huDUZfxoJ7wGMTffUE7vh1xePqef7gyrLJu9NApncqA","3mi1GmwEE3zo2jmfDuzvjSX9ovRXsDUKHvsntpkhuLJ9","BYxEJTDerkaRWBem3XgnVcdhppktBXa2HbkHPKj2Ui4Z"],"total":1016000}},"id":1}"#;

		let response: ClientResponse<GetSupplyResponse> = serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1114);
		check!(
			response.result.value
				== RpcSupply {
					circulating: 16_000,
					non_circulating: 1_000_000,
					total: 1_016_000,
					non_circulating_accounts: vec![
						"FEy8pTbP5fEoqMV1GdTz83byuA8EKByqYat1PKDgVAq5".to_string(),
						"9huDUZfxoJ7wGMTffUE7vh1xePqef7gyrLJu9NApncqA".to_string(),
						"3mi1GmwEE3zo2jmfDuzvjSX9ovRXsDUKHvsntpkhuLJ9".to_string(),
						"BYxEJTDerkaRWBem3XgnVcdhppktBXa2HbkHPKj2Ui4Z".to_string()
					]
				}
		);
	}
}
