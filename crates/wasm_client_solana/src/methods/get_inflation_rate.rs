use derive_more::derive::Into;
use serde::Deserialize;
use serde::Serialize;

use crate::impl_http_method;
use crate::rpc_response::RpcInflationRate;

/// Request for the `getInflationRate` RPC method, which returns the inflation
/// rate for the current epoch.
#[derive(Debug, Serialize)]
pub struct GetInflationRateRequest;

impl_http_method!(GetInflationRateRequest, "getInflationRate");

/// Response for the `getInflationRate` RPC method.
#[derive(Debug, Deserialize, Into)]
pub struct GetInflationRateResponse(RpcInflationRate);

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
			.method(GetInflationRateRequest::NAME)
			.id(1)
			.params(GetInflationRateRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getInflationRate"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"epoch":100,"foundation":0.001,"total":0.149,"validator":0.148},"id":1}"#;

		let response: ClientResponse<GetInflationRateResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		let value = response.result.0;
		assert!(approx_eq(value.foundation, 0.001));
		check!(value.epoch == 100);
		assert!(approx_eq(value.total, 0.149));
		assert!(approx_eq(value.validator, 0.148));
	}

	fn approx_eq(a: f64, b: f64) -> bool {
		const EPSILON: f64 = 1e-6;
		(a - b).abs() < EPSILON
	}
}
