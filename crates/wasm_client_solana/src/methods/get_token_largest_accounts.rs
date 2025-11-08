use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetTokenLargestAccountsRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub config: Option<CommitmentConfig>,
}

impl_http_method!(GetTokenLargestAccountsRequest, "getTokenLargestAccounts");

impl GetTokenLargestAccountsRequest {
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

#[serde_as]
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TokenLargestAccountsValue {
	#[serde_as(as = "DisplayFromStr")]
	pub address: Pubkey,
	pub amount: String,
	pub decimals: u8,
	pub ui_amount: Option<f64>,
	pub ui_amount_string: String,
}

impl Eq for TokenLargestAccountsValue {}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetTokenLargestAccountsResponse {
	pub context: Context,
	pub value: Vec<TokenLargestAccountsValue>,
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
			.method(GetTokenLargestAccountsRequest::NAME)
			.id(1)
			.params(GetTokenLargestAccountsRequest::new(pubkey!(
				"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"
			)))
			.build();
		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getTokenLargestAccounts",
    "params": [
      "3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1114},"value":[{"address":"FYjHNoFtSQ5uijKrZFyYAxvEr87hsKXkXcxkcmkBAf4r","amount":"771","decimals":2,"uiAmount":7.71,"uiAmountString":"7.71"},{"address":"BnsywxTcaYeNUtzrPxQUvzAWxfzZe3ZLUJ4wMMuLESnu","amount":"229","decimals":2,"uiAmount":2.29,"uiAmountString":"2.29"}]},"id":1}"#;

		let response: ClientResponse<GetTokenLargestAccountsResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1114);
		check!(
			response.result.value
				== vec![
					TokenLargestAccountsValue {
						address: pubkey!("FYjHNoFtSQ5uijKrZFyYAxvEr87hsKXkXcxkcmkBAf4r"),
						amount: "771".to_string(),
						ui_amount_string: "7.71".to_string(),
						decimals: 2,
						ui_amount: Some(7.71)
					},
					TokenLargestAccountsValue {
						address: pubkey!("BnsywxTcaYeNUtzrPxQUvzAWxfzZe3ZLUJ4wMMuLESnu"),
						amount: "229".to_string(),
						ui_amount_string: "2.29".to_string(),
						decimals: 2,
						ui_amount: Some(2.29)
					}
				]
		);
	}
}
