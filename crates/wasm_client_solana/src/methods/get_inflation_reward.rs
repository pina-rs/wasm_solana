use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use crate::impl_http_method;
use crate::rpc_config::RpcEpochConfig;
use crate::rpc_response::RpcInflationReward;

/// Request for the `getInflationReward` RPC method, which returns the inflation
/// reward paid to each address for an epoch.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetInflationRewardRequest {
	/// Base58 pubkeys of the accounts to query rewards for.
	#[serde_as(as = "Vec<DisplayFromStr>")]
	pub addresses: Vec<Pubkey>,
	/// Config selecting the epoch, commitment, and minimum context slot of the
	/// query. Defaults to the previous epoch.
	pub config: Option<RpcEpochConfig>,
}

impl_http_method!(GetInflationRewardRequest, "getInflationReward");

impl GetInflationRewardRequest {
	/// Creates a request for the given addresses using the default epoch
	/// config.
	pub fn new(addresses: Vec<Pubkey>) -> Self {
		Self {
			addresses,
			config: None,
		}
	}

	/// Creates a request for the given addresses with an explicit epoch config.
	pub fn new_with_config(addresses: Vec<Pubkey>, config: RpcEpochConfig) -> Self {
		Self {
			addresses,
			config: Some(config),
		}
	}
}

/// Response for the `getInflationReward` RPC method.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetInflationRewardResponse(Vec<Option<RpcInflationReward>>);

impl From<GetInflationRewardResponse> for Vec<Option<RpcInflationReward>> {
	fn from(value: GetInflationRewardResponse) -> Self {
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
			.method(GetInflationRewardRequest::NAME)
			.id(1)
			.params(GetInflationRewardRequest::new_with_config(
				vec![
					pubkey!("6dmNQ5jwLeLk5REvio1JcMshcbvkYMwy26sJ8pbkvStu"),
					pubkey!("BGsqMegLpV6n6Ve146sSX2dTjUMj3M92HnU8BbNRMhF2"),
				],
				RpcEpochConfig {
					epoch: Some(2),
					..Default::default()
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getInflationReward",
    "params": [
      [
        "6dmNQ5jwLeLk5REvio1JcMshcbvkYMwy26sJ8pbkvStu",
        "BGsqMegLpV6n6Ve146sSX2dTjUMj3M92HnU8BbNRMhF2"
      ],
      {
        "epoch": 2
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"amount":2500,"effectiveSlot":224,"epoch":2,"postBalance":499999442500},null],"id":1}"#;

		let response: ClientResponse<GetInflationRewardResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		let value = response.result.0;
		check!(value.len() == 2);
		let inflation_reward = value[0].as_ref().unwrap();
		check!(inflation_reward.amount == 2500);
		check!(inflation_reward.effective_slot == 224);
		check!(inflation_reward.epoch == 2);
		check!(inflation_reward.post_balance == 499_999_442_500);
	}
}
