use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use solana_vote_interface::state::MAX_LOCKOUT_HISTORY;

use crate::impl_http_method;

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct GetBlockCommitmentRequest {
	pub slot: u64,
}

impl_http_method!(GetBlockCommitmentRequest, "getBlockCommitment");

impl GetBlockCommitmentRequest {
	pub fn new(slot: u64) -> Self {
		Self { slot }
	}
}

type BlockCommitmentArray = [u64; MAX_LOCKOUT_HISTORY + 1];

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetBlockCommitmentResponse {
	pub commitment: Option<BlockCommitmentArray>,
	pub total_stake: u64,
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
			.method(GetBlockCommitmentRequest::NAME)
			.id(1)
			.params(GetBlockCommitmentRequest::new(5))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getBlockCommitment", "params": [5]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"commitment":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,10,32],"totalStake":42},"id":1}"#;

		let response: ClientResponse<GetBlockCommitmentResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");

		check!(response.result.total_stake == 42);
		check!(
			response.result.commitment
				== Some([
					0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
					0, 0, 0, 0, 10, 32
				])
		);
	}
}
