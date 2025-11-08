use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use super::impl_websocket_notification;
use crate::impl_http_method;
use crate::rpc_config::RpcKeyedAccount;
use crate::rpc_config::RpcProgramAccountsConfig;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetProgramAccountsRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub config: Option<RpcProgramAccountsConfig>,
}

impl_http_method!(GetProgramAccountsRequest, "getProgramAccounts");

impl GetProgramAccountsRequest {
	pub fn new(pubkey: Pubkey) -> Self {
		Self {
			pubkey,
			config: None,
		}
	}

	pub fn new_with_config(pubkey: Pubkey, config: RpcProgramAccountsConfig) -> Self {
		Self {
			pubkey,
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetProgramAccountsResponse(Option<Vec<RpcKeyedAccount>>);

impl_websocket_notification!(GetProgramAccountsResponse, "program");

impl GetProgramAccountsResponse {
	pub fn keyed_accounts(&self) -> Option<&Vec<RpcKeyedAccount>> {
		self.0.as_ref()
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
	use crate::rpc_config::RpcAccountInfoConfig;
	use crate::rpc_filter::Memcmp;
	use crate::rpc_filter::MemcmpEncodedBytes;
	use crate::rpc_filter::RpcFilterType;
	use crate::solana_account_decoder::UiAccount;
	use crate::solana_account_decoder::UiAccountData;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetProgramAccountsRequest::NAME)
			.id(1)
			.params(GetProgramAccountsRequest::new_with_config(
				pubkey!("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T"),
				RpcProgramAccountsConfig {
					filters: Some(vec![
						RpcFilterType::DataSize(17),
						RpcFilterType::Memcmp(Memcmp::new(
							4,
							MemcmpEncodedBytes::Base64("3Mc6vR".to_string()),
						)),
					]),
					account_config: RpcAccountInfoConfig::default(),
					with_context: None,
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getProgramAccounts",
    "params": [
      "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T",
      {
        "filters": [
          {
            "dataSize": 17
          },
          {
            "memcmp": {
              "bytes": "3Mc6vR",
              "encoding": "base64",
              "offset": 4
            }
          }
        ]
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"account":{"data":"2R9jLfiAQ9bgdcw6h8s44439","executable":false,"lamports":15298080,"owner":"4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T","rentEpoch":28,"space":42},"pubkey":"CxELquR1gPP8wHe33gZ4QxqGB3sZ9RSwsJ2KshVewkFY"}],"id":1}"#;

		let response: ClientResponse<GetProgramAccountsResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		let value = response.result.0.unwrap();
		check!(
			value
				== vec![RpcKeyedAccount {
					account: UiAccount {
						executable: false,
						data: UiAccountData::LegacyBinary("2R9jLfiAQ9bgdcw6h8s44439".to_string()),
						lamports: 15_298_080,
						owner: pubkey!("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T"),
						rent_epoch: 28,
						space: Some(42)
					},
					pubkey: pubkey!("CxELquR1gPP8wHe33gZ4QxqGB3sZ9RSwsJ2KshVewkFY")
				}]
		);
	}
}
