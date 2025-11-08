use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;

use crate::impl_http_method;
use crate::rpc_config::RpcGetVoteAccountsConfig;
use crate::rpc_response::RpcVoteAccountStatus;

#[skip_serializing_none]
#[derive(Debug, Default, Serialize_tuple)]
pub struct GetVoteAccountsRequest {
	pub config: Option<RpcGetVoteAccountsConfig>,
}

impl_http_method!(GetVoteAccountsRequest, "getVoteAccounts");

impl GetVoteAccountsRequest {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_config(config: RpcGetVoteAccountsConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetVoteAccountsResponse(RpcVoteAccountStatus);

impl From<GetVoteAccountsResponse> for RpcVoteAccountStatus {
	fn from(value: GetVoteAccountsResponse) -> Self {
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
	use crate::rpc_response::RpcVoteAccountInfo;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetVoteAccountsRequest::NAME)
			.id(1)
			.params(GetVoteAccountsRequest::new_with_config(
				RpcGetVoteAccountsConfig {
					vote_pubkey: Some(pubkey!("3ZT31jkAGhUaw8jsy4bTknwBMP8i4Eueh52By4zXcsVw")),
					..Default::default()
				},
			))
			.build();
		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getVoteAccounts",
    "params": [
      {
        "votePubkey": "3ZT31jkAGhUaw8jsy4bTknwBMP8i4Eueh52By4zXcsVw"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"current":[{"commission":0,"epochVoteAccount":true,"epochCredits":[[1,64,0],[2,192,64]],"nodePubkey":"B97CCUW3AEZFGy6uUg6zUdnNYvnVq5VG8PUtb2HayTDD","lastVote":147,"activatedStake":42,"votePubkey":"3ZT31jkAGhUaw8jsy4bTknwBMP8i4Eueh52By4zXcsVw"}],"delinquent":[]},"id":1}"#;

		let response: ClientResponse<GetVoteAccountsResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(
			response.result.0
				== RpcVoteAccountStatus {
					current: vec![RpcVoteAccountInfo {
						activated_stake: 42,
						vote_pubkey: pubkey!("3ZT31jkAGhUaw8jsy4bTknwBMP8i4Eueh52By4zXcsVw"),
						node_pubkey: pubkey!("B97CCUW3AEZFGy6uUg6zUdnNYvnVq5VG8PUtb2HayTDD"),
						commission: 0,
						epoch_vote_account: true,
						epoch_credits: vec![(1, 64, 0), (2, 192, 64)],
						last_vote: 147,
						root_slot: 0
					}],
					delinquent: vec![]
				}
		);
	}
}
