use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_sdk::clock::Slot;
use solana_commitment_config::CommitmentConfig;

use crate::impl_http_method;

#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(rename_all = "camelCase")]
pub struct GetBlocksWithLimitRequest {
	pub start_slot: Slot,
	pub limit: usize,
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetBlocksWithLimitRequest, "getBlocksWithLimit");

impl GetBlocksWithLimitRequest {
	pub fn new(start_slot: Slot, limit: usize) -> Self {
		Self {
			start_slot,
			limit,
			config: None,
		}
	}

	pub fn new_with_config(start_slot: Slot, limit: usize, config: CommitmentConfig) -> Self {
		Self {
			start_slot,
			limit,
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetBlocksWithLimitResponse(Vec<Slot>);

impl From<GetBlocksWithLimitResponse> for Vec<Slot> {
	fn from(value: GetBlocksWithLimitResponse) -> Self {
		value.0
	}
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
			.method(GetBlocksWithLimitRequest::NAME)
			.id(1)
			.params(GetBlocksWithLimitRequest::new(5, 3))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getBlocksWithLimit", "params": [5, 3]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[5,6,7],"id":1}"#;

		let response: ClientResponse<GetBlocksWithLimitResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == vec![5, 6, 7]);
	}
}
