use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcBlockProductionConfig;
use crate::rpc_response::RpcBlockProduction;

/// Request for the `getBlockProduction` RPC method, which returns recent block
/// production statistics, optionally scoped to an identity or slot range.
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple, Default)]
pub struct GetBlockProductionRequest {
	/// Config selecting the identity and slot range to report on. Defaults to
	/// the current epoch for all identities.
	pub config: Option<RpcBlockProductionConfig>,
}

impl_http_method!(GetBlockProductionRequest, "getBlockProduction");

impl GetBlockProductionRequest {
	/// Creates a request for the full current-epoch block production stats.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a request scoped by the given block production config.
	pub fn new_with_config(config: RpcBlockProductionConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

/// Response for the `getBlockProduction` RPC method.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetBlockProductionResponse {
	/// The slot that the RPC node used to evaluate the request.
	pub context: Context,
	/// Per-identity leader slot counts and the slot range they cover.
	pub value: RpcBlockProduction,
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use assert2::check;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::rpc_response::RpcBlockProductionRange;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetBlockProductionRequest::NAME)
			.id(1)
			.params(GetBlockProductionRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getBlockProduction"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":9887},"value":{"byIdentity":{"85iYT5RuzRTDgjyRa3cP8SYhM2j21fj7NhfJ3peu1DPr":[9888,9886]},"range":{"firstSlot":0,"lastSlot":9887}}},"id":1}"#;

		let response: ClientResponse<GetBlockProductionResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 9887);

		let value = response.result.value;
		check!(
			value.by_identity
				== HashMap::from_iter([(
					"85iYT5RuzRTDgjyRa3cP8SYhM2j21fj7NhfJ3peu1DPr".to_string(),
					(9888, 9886)
				)])
		);
		check!(
			value.range
				== RpcBlockProductionRange {
					first_slot: 0,
					last_slot: 9887
				}
		);
	}
}
