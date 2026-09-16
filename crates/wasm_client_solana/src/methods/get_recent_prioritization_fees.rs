use serde::Deserialize;
use serde::Serializer;
use serde_tuple::Serialize_tuple;
use solana_pubkey::Pubkey;

use crate::impl_http_method;
use crate::rpc_response::RpcPrioritizationFee;

// serde's `serialize_with` requires exactly this signature for an `Option`
// field.
#[allow(clippy::ref_option)]
fn serialize_pubkey_strings<S: Serializer>(
	accounts: &Option<Vec<Pubkey>>,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	match accounts {
		Some(pubkeys) => serializer.collect_seq(pubkeys.iter().map(Pubkey::to_string)),
		None => serializer.serialize_none(),
	}
}

/// Request for the `getRecentPrioritizationFees` RPC method, which returns the
/// per-slot prioritization fees paid by recent transactions.
#[derive(Debug, Serialize_tuple)]
pub struct GetRecentPrioritizationFeesRequest {
	#[serde(serialize_with = "serialize_pubkey_strings")]
	accounts: Option<Vec<Pubkey>>,
}

impl_http_method!(
	GetRecentPrioritizationFeesRequest,
	"getRecentPrioritizationFees"
);

impl Default for GetRecentPrioritizationFeesRequest {
	fn default() -> Self {
		Self::new()
	}
}

impl GetRecentPrioritizationFeesRequest {
	/// Creates a request for the fees of all accounts.
	pub fn new() -> Self {
		GetRecentPrioritizationFeesRequest { accounts: None }
	}

	/// Creates a request for the fees paid by transactions that lock the given
	/// accounts.
	pub fn new_with_accounts(accounts: Vec<Pubkey>) -> Self {
		GetRecentPrioritizationFeesRequest {
			accounts: Some(accounts),
		}
	}
}

/// Response for the `getRecentPrioritizationFees` RPC method.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetRecentPrioritizationFeesResponse(Vec<RpcPrioritizationFee>);

impl From<GetRecentPrioritizationFeesResponse> for Vec<RpcPrioritizationFee> {
	fn from(value: GetRecentPrioritizationFeesResponse) -> Self {
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
			.method(GetRecentPrioritizationFeesRequest::NAME)
			.id(1)
			.params(GetRecentPrioritizationFeesRequest::new_with_accounts(vec![
				pubkey!("CxELquR1gPP8wHe33gZ4QxqGB3sZ9RSwsJ2KshVewkFY"),
			]))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getRecentPrioritizationFees",
    "params": [
      [
        "CxELquR1gPP8wHe33gZ4QxqGB3sZ9RSwsJ2KshVewkFY"
      ]
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"slot":348125,"prioritizationFee":0},{"slot":348126,"prioritizationFee":1000},{"slot":348127,"prioritizationFee":500},{"slot":348128,"prioritizationFee":0},{"slot":348129,"prioritizationFee":1234}],"id":1}"#;

		let response: ClientResponse<GetRecentPrioritizationFeesResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(
			response.result.0
				== vec![
					RpcPrioritizationFee {
						slot: 348_125,
						prioritization_fee: 0,
					},
					RpcPrioritizationFee {
						slot: 348_126,
						prioritization_fee: 1000
					},
					RpcPrioritizationFee {
						slot: 348_127,
						prioritization_fee: 500
					},
					RpcPrioritizationFee {
						slot: 348_128,
						prioritization_fee: 0
					},
					RpcPrioritizationFee {
						slot: 348_129,
						prioritization_fee: 1234
					}
				]
		);
	}
}
