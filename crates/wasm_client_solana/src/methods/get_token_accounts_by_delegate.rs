use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcAccountInfoConfig;
use crate::rpc_config::RpcKeyedAccount;
use crate::rpc_config::RpcTokenAccountsFilter;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetTokenAccountsByDelegateRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub filter: RpcTokenAccountsFilter,
	pub config: Option<RpcAccountInfoConfig>,
}

impl_http_method!(
	GetTokenAccountsByDelegateRequest,
	"getTokenAccountsByDelegate"
);

impl GetTokenAccountsByDelegateRequest {
	pub fn new_mint(pubkey: Pubkey, account_key: Pubkey) -> Self {
		Self {
			pubkey,
			filter: RpcTokenAccountsFilter::Mint(account_key),
			config: None,
		}
	}

	pub fn new_mint_with_config(
		pubkey: Pubkey,
		account_key: Pubkey,
		config: RpcAccountInfoConfig,
	) -> Self {
		Self {
			pubkey,
			filter: RpcTokenAccountsFilter::Mint(account_key),
			config: Some(config),
		}
	}

	pub fn new_program(pubkey: Pubkey, account_key: Pubkey) -> Self {
		Self {
			pubkey,
			filter: RpcTokenAccountsFilter::ProgramId(account_key),
			config: None,
		}
	}

	pub fn new_program_with_config(
		pubkey: Pubkey,
		account_key: Pubkey,
		config: RpcAccountInfoConfig,
	) -> Self {
		Self {
			pubkey,
			filter: RpcTokenAccountsFilter::ProgramId(account_key),
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetTokenAccountsByDelegateResponse {
	pub context: Context,
	pub value: Option<Vec<RpcKeyedAccount>>,
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
			.method(GetTokenAccountsByDelegateRequest::NAME)
			.id(1)
			.params(GetTokenAccountsByDelegateRequest::new_program_with_config(
				pubkey!("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T"),
				pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
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
    "method": "getTokenAccountsByDelegate",
    "params": [
      "4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T",
      {
        "programId": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
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
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1114},"value":[{"account":{"data":{"program":"spl-token","parsed":{"info":{"tokenAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"delegate":"4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T","delegatedAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"state":"initialized","isNative":false,"mint":"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E","owner":"CnPoSPKXu7wJqxe59Fs72tkBeALovhsCxYeFwPCQH9TD"},"type":"account"},"space":165},"executable":false,"lamports":1726080,"owner":"TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA","rentEpoch":4,"space":165},"pubkey":"28YTZEwqtMHWrhWcvv34se7pjS7wctgqzCPB3gReCFKp"}]},"id":1}"#;

		let response: ClientResponse<GetTokenAccountsByDelegateResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1114);
		check!(
            response.result.value==
            Some(
            vec![RpcKeyedAccount {
                account: UiAccount {
                    owner: pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
                    data: UiAccountData::Json(ParsedAccount {
                        program: "spl-token".to_string(),
                        space: 165,
                        parsed: serde_json::from_str(r#"{"info":{"tokenAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"delegate":"4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T","delegatedAmount":{"amount":"1","decimals":1,"uiAmount":0.1,"uiAmountString":"0.1"},"state":"initialized","isNative":false,"mint":"3wyAj7Rt1TWVPZVteFJPLa26JmLvdb1CAKEFZm3NY75E","owner":"CnPoSPKXu7wJqxe59Fs72tkBeALovhsCxYeFwPCQH9TD"},"type":"account"}"#).unwrap()
                    }),
                    executable: false,
                    lamports: 1_726_080,
                    rent_epoch: 4,
                    space: Some(165)
                },
                pubkey: pubkey!("28YTZEwqtMHWrhWcvv34se7pjS7wctgqzCPB3gReCFKp")
            }])
        );
	}
}
