use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_epoch_info::EpochInfo;

use crate::impl_http_method;

#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple, Default)]
pub struct GetEpochInfoRequest {
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetEpochInfoRequest, "getEpochInfo");

impl GetEpochInfoRequest {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_config(config: CommitmentConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetEpochInfoResponse(EpochInfo);

impl From<GetEpochInfoResponse> for EpochInfo {
	fn from(value: GetEpochInfoResponse) -> Self {
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
			.method(GetEpochInfoRequest::NAME)
			.id(1)
			.params(GetEpochInfoRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getEpochInfo"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"absoluteSlot":166598,"blockHeight":166500,"epoch":27,"slotIndex":2790,"slotsInEpoch":8192,"transactionCount":22661093},"id":1}"#;
		let response: ClientResponse<GetEpochInfoResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(
			response.result.0
				== EpochInfo {
					absolute_slot: 166_598,
					block_height: 166_500,
					epoch: 27,
					slot_index: 2790,
					slots_in_epoch: 8192,
					transaction_count: Some(22_661_093)
				}
		);
	}
}
