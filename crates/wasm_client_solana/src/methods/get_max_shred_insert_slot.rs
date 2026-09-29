use derive_more::derive::From;
use derive_more::derive::Into;
use serde::Deserialize;
use serde::Serialize;

use crate::impl_http_method;

/// Request for the `getMaxShredInsertSlot` RPC method, which returns the
/// highest slot the node has inserted a shred for. The shred insert stage runs
/// ahead of replay, so this leads `getMaxRetransmitSlot` and `getSlot` —
/// useful for measuring how far behind a node's own processing is.
#[derive(Debug, Serialize)]
pub struct GetMaxShredInsertSlotRequest;

impl_http_method!(GetMaxShredInsertSlotRequest, "getMaxShredInsertSlot");

/// Response for the `getMaxShredInsertSlot` RPC method.
#[derive(Debug, Deserialize, From, Into)]
pub struct GetMaxShredInsertSlotResponse(u64);

#[cfg(test)]
mod tests {

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetMaxShredInsertSlotRequest::NAME)
			.id(1)
			.params(GetMaxShredInsertSlotRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getMaxShredInsertSlot"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{ "jsonrpc": "2.0", "result": 1234, "id": 1 }"#;

		let response: ClientResponse<GetMaxShredInsertSlotResponse> =
			serde_json::from_str(raw_json).unwrap();

		assert_eq!(response.id, 1);
		assert_eq!(response.jsonrpc, "2.0");
		assert_eq!(response.result.0, 1234);
	}
}
