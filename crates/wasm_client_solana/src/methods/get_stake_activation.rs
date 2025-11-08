use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use crate::impl_http_method;
use crate::rpc_config::RpcEpochConfig;
use crate::rpc_response::RpcStakeActivation;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetStakeActivationRequest {
	#[serde_as(as = "DisplayFromStr")]
	pubkey: Pubkey,
	config: Option<RpcEpochConfig>,
}

impl_http_method!(GetStakeActivationRequest, "getStakeActivation");

impl GetStakeActivationRequest {
	pub fn new(pubkey: Pubkey) -> Self {
		Self {
			pubkey,
			config: None,
		}
	}

	pub fn new_with_config(pubkey: Pubkey, config: RpcEpochConfig) -> Self {
		Self {
			pubkey,
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetStakeActivationResponse(RpcStakeActivation);

impl From<GetStakeActivationResponse> for RpcStakeActivation {
	fn from(value: GetStakeActivationResponse) -> Self {
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
	use crate::rpc_response::StakeActivationState;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetStakeActivationRequest::NAME)
			.id(1)
			.params(GetStakeActivationRequest::new_with_config(
				pubkey!("CYRJWqiSjLitBAcRxPvWpgX3s5TvmN2SuRY3eEYypFvT"),
				RpcEpochConfig {
					epoch: Some(4),
					..Default::default()
				},
			))
			.build();
		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getStakeActivation",
    "params": [
      "CYRJWqiSjLitBAcRxPvWpgX3s5TvmN2SuRY3eEYypFvT",
      {
        "epoch": 4
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"active":124429280,"inactive":73287840,"state":"activating"},"id":1}"#;

		let response: ClientResponse<GetStakeActivationResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(
			response.result.0
				== RpcStakeActivation {
					active: 124_429_280,
					inactive: 73_287_840,
					state: StakeActivationState::Activating
				}
		);
	}
}
