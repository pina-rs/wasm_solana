use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcAccountInfoConfig;
use crate::solana_account_decoder::UiAccount;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
#[serde(rename_all = "camelCase")]
pub struct GetMultipleAccountsRequest {
	#[serde_as(as = "Vec<DisplayFromStr>")]
	pub addresses: Vec<Pubkey>,
	pub config: Option<RpcAccountInfoConfig>,
}

impl_http_method!(GetMultipleAccountsRequest, "getMultipleAccounts");

impl GetMultipleAccountsRequest {
	pub fn new(addresses: Vec<Pubkey>) -> Self {
		Self {
			addresses,
			config: None,
		}
	}

	pub fn new_with_config(addresses: Vec<Pubkey>, config: RpcAccountInfoConfig) -> Self {
		Self {
			addresses,
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetMultipleAccountsResponse {
	pub context: Context,
	pub value: Vec<Option<UiAccount>>,
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_pubkey::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_account_decoder::UiAccountData;
	use crate::solana_account_decoder::UiAccountEncoding;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetMultipleAccountsRequest::NAME)
			.id(1)
			.params(GetMultipleAccountsRequest::new_with_config(
				vec![
					pubkey!("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg"),
					pubkey!("4fYNw3dojWmQ4dXtSGE9epjRGy9pFSx62YypT7avPYvA"),
				],
				RpcAccountInfoConfig {
					encoding: Some(UiAccountEncoding::Base58),
					..Default::default()
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getMultipleAccounts",
    "params": [
      [
        "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg",
        "4fYNw3dojWmQ4dXtSGE9epjRGy9pFSx62YypT7avPYvA"
      ],
      {
        "encoding": "base58"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1},"value":[{"data":["","base64"],"executable":false,"lamports":1000000000,"owner":"11111111111111111111111111111111","rentEpoch":2,"space":16},{"data":["","base64"],"executable":false,"lamports":5000000000,"owner":"11111111111111111111111111111111","rentEpoch":2,"space":0}]},"id":1}"#;

		let response: ClientResponse<GetMultipleAccountsResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1);
		let value = response.result.value;
		check!(
			value
				== vec![
					Some(UiAccount {
						lamports: 1_000_000_000,
						space: Some(16),
						data: UiAccountData::Binary(String::new(), UiAccountEncoding::Base64),
						owner: Pubkey::default(),
						executable: false,
						rent_epoch: 2
					}),
					Some(UiAccount {
						lamports: 5_000_000_000,
						space: Some(0),
						data: UiAccountData::Binary(String::new(), UiAccountEncoding::Base64),
						owner: Pubkey::default(),
						executable: false,
						rent_epoch: 2
					})
				]
		);
	}
}
