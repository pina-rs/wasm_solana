use serde::Deserialize;
use serde::Serialize;

use crate::impl_http_method;

/// Request for the `getHealth` RPC method, which reports whether the node is
/// healthy enough to respond to RPC requests.
#[derive(Debug, Serialize)]
pub struct GetHealthRequest;

impl_http_method!(GetHealthRequest, "getHealth");

/// JSON-RPC error value returned by an unhealthy node.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct ErrorValue {
	/// JSON-RPC error code.
	pub code: i32,
	/// Human readable error message.
	pub message: String,
	/// Additional error data, such as the number of slots behind the node is.
	pub data: serde_json::Value,
}

/// Response for the `getHealth` RPC method: `ok` when the node is healthy.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetHealthResponse(pub String);

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
			.method(GetHealthRequest::NAME)
			.id(1)
			.params(GetHealthRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getHealth"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{ "jsonrpc": "2.0", "result": "ok", "id": 1 }"#;

		let response: ClientResponse<GetHealthResponse> = serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == "ok");
	}
}
