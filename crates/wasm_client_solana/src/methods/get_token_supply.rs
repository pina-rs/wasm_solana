use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;
use crate::solana_account_decoder::parse_token::UiTokenAmount;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetTokenSupplyRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetTokenSupplyRequest, "getTokenSupply");

impl GetTokenSupplyRequest {
	pub fn new(pubkey: Pubkey) -> Self {
		Self {
			pubkey,
			config: None,
		}
	}

	pub fn new_with_config(pubkey: Pubkey, config: CommitmentConfig) -> Self {
		Self {
			pubkey,
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetTokenSupplyResponse {
	pub context: Context,
	pub value: UiTokenAmount,
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_sdk::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetTokenSupplyRequest::NAME)
			.id(1)
			.params(GetTokenSupplyRequest::new(pubkey!(
				"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"
			)))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getTokenSupply", "params": ["3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1114},"value":{"amount":"100000","decimals":2,"uiAmount":1000,"uiAmountString":"1000"}},"id":1}"#;

		let response: ClientResponse<GetTokenSupplyResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1114);
		check!(
			response.result.value
				== UiTokenAmount {
					amount: "100000".to_string(),
					ui_amount_string: "1000".to_string(),
					decimals: 2,
					ui_amount: Some(1000.0),
				}
		);
	}
}
