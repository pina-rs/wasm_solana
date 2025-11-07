use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;

use crate::impl_http_method;
use crate::rpc_response::RpcInflationGovernor;

#[skip_serializing_none]
#[derive(Debug, Default, Serialize_tuple)]
pub struct GetInflationGovernorRequest {
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetInflationGovernorRequest, "getInflationGovernor");

impl GetInflationGovernorRequest {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_config(config: CommitmentConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetInflationGovernorResponse(RpcInflationGovernor);

impl From<GetInflationGovernorResponse> for RpcInflationGovernor {
	fn from(value: GetInflationGovernorResponse) -> Self {
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
			.method(GetInflationGovernorRequest::NAME)
			.id(1)
			.params(GetInflationGovernorRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getInflationGovernor"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"foundation":0.05,"foundationTerm":7,"initial":0.15,"taper":0.15,"terminal":0.015},"id":1}"#;

		let response: ClientResponse<GetInflationGovernorResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		let value = response.result.0;
		check!(approx_eq(value.foundation, 0.05));
		check!(approx_eq(value.foundation_term, 7.0));
		check!(approx_eq(value.initial, 0.15));
		check!(approx_eq(value.taper, 0.15));
		check!(approx_eq(value.terminal, 0.015));
	}

	fn approx_eq(a: f64, b: f64) -> bool {
		const EPSILON: f64 = 1e-6;
		(a - b).abs() < EPSILON
	}
}
