use derive_more::derive::From;
use derive_more::derive::Into;
use serde::Deserialize;
use serde::Serialize;

use crate::impl_http_method;

/// Request for the `getMaxRetransmitSlot` RPC method, which returns the highest
/// slot the node has received a shred for.
#[derive(Debug, Serialize)]
pub struct GetMaxRetransmitSlotRequest;

impl_http_method!(GetMaxRetransmitSlotRequest, "getMaxRetransmitSlot");

/// Response for the `getMaxRetransmitSlot` RPC method.
#[derive(Debug, Deserialize, From, Into)]
pub struct GetMaxRetransmitSlotResponse(u64);

#[cfg(test)]
mod tests {

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetMaxRetransmitSlotRequest::NAME)
			.id(1)
			.params(GetMaxRetransmitSlotRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getMaxRetransmitSlot"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{ "jsonrpc": "2.0", "result": 1234, "id": 1 }"#;

		let response: ClientResponse<GetMaxRetransmitSlotResponse> =
			serde_json::from_str(raw_json).unwrap();

		assert_eq!(response.id, 1);
		assert_eq!(response.jsonrpc, "2.0");
		assert_eq!(response.result.0, 1234);
	}
}
