use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;

use crate::impl_http_method;

/// Request for the `getMinimumBalanceForRentExemption` RPC method, which
/// returns the lamport balance an account must hold to be rent exempt.
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetMinimumBalanceForRentExemptionRequest {
	/// Account data length in bytes that the exempt balance is computed for.
	pub data_length: usize,
	/// Commitment level for the request. Defaults to the client's commitment
	/// when omitted.
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(
	GetMinimumBalanceForRentExemptionRequest,
	"getMinimumBalanceForRentExemption"
);

impl GetMinimumBalanceForRentExemptionRequest {
	/// Creates a request that evaluates the exemption at the client's default
	/// commitment.
	pub fn new(data_length: usize) -> Self {
		Self {
			data_length,
			config: None,
		}
	}

	/// Creates a request that evaluates the exemption at the given commitment
	/// level.
	pub fn new_with_config(data_length: usize, config: CommitmentConfig) -> Self {
		Self {
			data_length,
			config: Some(config),
		}
	}
}

/// Response for the `getMinimumBalanceForRentExemption` RPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMinimumBalanceForRentExemptionResponse(u64);

impl From<GetMinimumBalanceForRentExemptionResponse> for u64 {
	fn from(val: GetMinimumBalanceForRentExemptionResponse) -> Self {
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
			.method(GetMinimumBalanceForRentExemptionRequest::NAME)
			.id(1)
			.params(GetMinimumBalanceForRentExemptionRequest::new(50))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getMinimumBalanceForRentExemption", "params": [50]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{ "jsonrpc": "2.0", "result": 500, "id": 1 }"#;

		let response: ClientResponse<GetMinimumBalanceForRentExemptionResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == 500);
	}
}
