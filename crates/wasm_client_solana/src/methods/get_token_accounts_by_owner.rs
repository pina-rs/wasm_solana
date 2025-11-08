use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcAccountInfoConfig;
use crate::rpc_config::RpcKeyedAccount;
use crate::rpc_config::RpcTokenAccountsFilter;

#[serde_as]
#[derive(Debug, Serialize_tuple)]
pub struct GetTokenAccountsByOwnerRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub owner: Pubkey,
	pub filter: RpcTokenAccountsFilter,
	pub config: Option<RpcAccountInfoConfig>,
}

impl_http_method!(GetTokenAccountsByOwnerRequest, "getTokenAccountsByOwner");

impl GetTokenAccountsByOwnerRequest {
	pub fn new(owner: Pubkey, filter: RpcTokenAccountsFilter) -> Self {
		Self {
			owner,
			filter,
			config: None,
		}
	}

	pub fn new_with_config(
		owner: Pubkey,
		filter: RpcTokenAccountsFilter,
		config: RpcAccountInfoConfig,
	) -> Self {
		Self {
			owner,
			filter,
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetTokenAccountsByOwnerResponse {
	pub context: Context,
	pub value: Vec<RpcKeyedAccount>,
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_pubkey::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_account_decoder::UiAccount;
	use crate::solana_account_decoder::UiAccountData;
	use crate::solana_account_decoder::UiAccountEncoding;
	use crate::solana_account_decoder::parse_account_data::ParsedAccount;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetTokenAccountsByOwnerRequest::NAME)
			.id(1)
			.params(GetTokenAccountsByOwnerRequest::new_with_config(
				pubkey!("4Qkev8aNZcqFNSRhQzwyLMFSsi94jHqE8WNVTJzTP99F"),
				RpcTokenAccountsFilter::Mint(pubkey!(
					"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"
				)),
				RpcAccountInfoConfig {
					encoding: Some(UiAccountEncoding::JsonParsed),
					..Default::default()
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getTokenAccountsByOwner",
    "params": [
      "4Qkev8aNZcqFNSRhQzwyLMFSsi94jHqE8WNVTJzTP99F",
      {
        "mint": "3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E"
      },
      {
        "encoding": "jsonParsed"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1114},"value":[{"account":{"data":{"program":"spl-token","parsed":{"accountType":"account","info":{"tokenAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"delegate":"4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T","delegatedAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"state":"initialized","isNative":false,"mint":"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E","owner":"4Qkev8aNZcqFNSRhQzwyLMFSsi94jHqE8WNVTJzTP99F"},"type":"account"},"space":165},"executable":false,"lamports":1726080,"owner":"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","rentEpoch":4,"space":165},"pubkey":"C2gJg6tKpQs41PRS1nC8aw3ZKNZK3HQQZGVrDFDup5nx"}]},"id":1}"#;

		let response: ClientResponse<GetTokenAccountsByOwnerResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1114);
		check!(
            response.result.value==
            vec![RpcKeyedAccount {
                account: UiAccount {
                    owner: pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
                    data: UiAccountData::Json(ParsedAccount {
                        program: "spl-token".to_string(),
                        space: 165,
                        parsed: serde_json::from_str(r#"{"accountType":"account","info":{"tokenAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"delegate":"4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T","delegatedAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"state":"initialized","isNative":false,"mint":"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E","owner":"4Qkev8aNZcqFNSRhQzwyLMFSsi94jHqE8WNVTJzTP99F"},"type":"account"}"#).unwrap()
                    }),
                    executable: false,
                    lamports: 1_726_080,
                    rent_epoch: 4,
                    space: Some(165)
                },
                pubkey: pubkey!("C2gJg6tKpQs41PRS1nC8aw3ZKNZK3HQQZGVrDFDup5nx"),
            }]
        );
	}
}
