use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;

use crate::impl_http_method;

#[derive(Debug, Default, Serialize_tuple)]
pub struct GetSlotLeaderRequest {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetSlotLeaderRequest, "getSlotLeader");

impl GetSlotLeaderRequest {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_config(config: CommitmentConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetSlotLeaderResponse(#[serde_as(as = "DisplayFromStr")] Pubkey);

impl From<GetSlotLeaderResponse> for Pubkey {
	fn from(val: GetSlotLeaderResponse) -> Self {
		val.0
	}
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_pubkey::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetSlotLeaderRequest::NAME)
			.id(1)
			.params(GetSlotLeaderRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getSlotLeader"}"###);
	}

	#[test]
	fn response() {
		let raw_json =
			r#"{"jsonrpc":"2.0","result":"ENvAW7JScgYq6o4zKZwewtkzzJgDzuJAFxYasvmEQdpS","id":1}"#;

		let response: ClientResponse<GetSlotLeaderResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == pubkey!("ENvAW7JScgYq6o4zKZwewtkzzJgDzuJAFxYasvmEQdpS"));
	}
}
