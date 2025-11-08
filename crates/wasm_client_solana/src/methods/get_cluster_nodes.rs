use serde::Deserialize;
use serde::Serialize;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use crate::impl_http_method;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetClusterNodesRequest;

impl_http_method!(GetClusterNodesRequest, "getClusterNodes");

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcContactInfoWasm {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub gossip: Option<String>,
	pub tpu: Option<String>,
	pub rpc: Option<String>,
	pub version: Option<String>,
	pub feature_set: Option<u32>,
	pub shred_version: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetClusterNodesResponse(Vec<RpcContactInfoWasm>);

impl From<GetClusterNodesResponse> for Vec<RpcContactInfoWasm> {
	fn from(value: GetClusterNodesResponse) -> Self {
		value.0
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
			.method(GetClusterNodesRequest::NAME)
			.id(1)
			.params(GetClusterNodesRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getClusterNodes"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"gossip":"10.239.6.48:8001","pubkey":"9QzsJf7LPLj8GkXbYT3LFDKqsj2hHG7TA3xinJHu8epQ","rpc":"10.239.6.48:8899","tpu":"10.239.6.48:8856","version":"1.0.0 c375ce1f"}],"id":1}"#;

		let response: ClientResponse<GetClusterNodesResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");

		let value = &response.result.0[0];

		check!(value.gossip.as_ref().unwrap() == "10.239.6.48:8001");
		check!(value.pubkey == pubkey!("9QzsJf7LPLj8GkXbYT3LFDKqsj2hHG7TA3xinJHu8epQ"));
		check!(value.rpc.as_ref().unwrap() == "10.239.6.48:8899");
		check!(value.tpu.as_ref().unwrap() == "10.239.6.48:8856");
		check!(value.version.as_ref().unwrap() == "1.0.0 c375ce1f");
		check!(value.feature_set.is_none());
		check!(value.shred_version.is_none());
	}
}
