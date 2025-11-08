use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use solana_clock::UnixTimestamp;

use crate::impl_http_method;

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct GetBlockTimeRequest {
	pub slot: u64,
}

impl_http_method!(GetBlockTimeRequest, "getBlockTime");

impl GetBlockTimeRequest {
	pub fn new(slot: u64) -> Self {
		Self { slot }
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetBlockTimeResponse(Option<UnixTimestamp>);

impl From<GetBlockTimeResponse> for Option<UnixTimestamp> {
	fn from(val: GetBlockTimeResponse) -> Self {
		val.0
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
			.method(GetBlockTimeRequest::NAME)
			.id(1)
			.params(GetBlockTimeRequest::new(5))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getBlockTime", "params": [5]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":1574721591,"id":1}"#;

		let response: ClientResponse<GetBlockTimeResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0.unwrap() == 1_574_721_591);
	}
}
