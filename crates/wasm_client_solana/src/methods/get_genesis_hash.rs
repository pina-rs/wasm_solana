use serde::Deserialize;
use serde::Serialize;

use crate::impl_http_method;

/// Request for the `getGenesisHash` RPC method, which returns the genesis hash
/// that identifies the cluster.
#[derive(Debug, Serialize)]
pub struct GetGenesisHashRequest;

impl_http_method!(GetGenesisHashRequest, "getGenesisHash");

/// Response for the `getGenesisHash` RPC method: the base58 genesis blockhash.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetGenesisHashResponse(String);

impl From<GetGenesisHashResponse> for String {
	fn from(val: GetGenesisHashResponse) -> Self {
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
			.method(GetGenesisHashRequest::NAME)
			.id(1)
			.params(GetGenesisHashRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getGenesisHash"}"###);
	}

	#[test]
	fn response() {
		let raw_json =
			r#"{"jsonrpc":"2.0","result":"GH7ome3EiwEr7tu9JuTh2dpYWBJK3z69Xm1ZE3MEE6JC","id":1}"#;

		let response: ClientResponse<GetGenesisHashResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");

		check!(response.result.0 == "GH7ome3EiwEr7tu9JuTh2dpYWBJK3z69Xm1ZE3MEE6JC");
	}
}
