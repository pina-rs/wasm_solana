use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;

use super::Context;
use crate::impl_http_method;
use crate::rpc_config::RpcLargestAccountsConfig;
use crate::rpc_response::RpcAccountBalance;

#[skip_serializing_none]
#[derive(Debug, Default, Serialize_tuple)]
pub struct GetLargestAccountsRequest {
	pub config: Option<RpcLargestAccountsConfig>,
}

impl_http_method!(GetLargestAccountsRequest, "getLargestAccounts");

impl GetLargestAccountsRequest {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn new_with_config(config: RpcLargestAccountsConfig) -> Self {
		Self {
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetLargestAccountsResponse {
	pub context: Context,
	pub value: Vec<RpcAccountBalance>,
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
			.method(GetLargestAccountsRequest::NAME)
			.id(1)
			.params(GetLargestAccountsRequest::new())
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getLargestAccounts"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":54},"value":[{"lamports":999974,"address":"99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ"},{"lamports":42,"address":"uPwWLo16MVehpyWqsLkK3Ka8nLowWvAHbBChqv2FZeL"},{"lamports":42,"address":"aYJCgU7REfu3XF8b3QhkqgqQvLizx8zxuLBHA25PzDS"},{"lamports":42,"address":"CTvHVtQ4gd4gUcw3bdVgZJJqApXE9nCbbbP4VTS5wE1D"},{"lamports":20,"address":"4fq3xJ6kfrh9RkJQsmVd5gNMvJbuSHfErywvEjNQDPxu"},{"lamports":4,"address":"AXJADheGVp9cruP8WYu46oNkRbeASngN5fPCMVGQqNHa"},{"lamports":2,"address":"8NT8yS6LiwNprgW4yM1jPPow7CwRUotddBVkrkWgYp24"},{"lamports":1,"address":"SysvarEpochSchedu1e111111111111111111111111"},{"lamports":1,"address":"11111111111111111111111111111111"},{"lamports":1,"address":"Stake11111111111111111111111111111111111111"},{"lamports":1,"address":"SysvarC1ock11111111111111111111111111111111"},{"lamports":1,"address":"StakeConfig11111111111111111111111111111111"},{"lamports":1,"address":"SysvarRent111111111111111111111111111111111"},{"lamports":1,"address":"Config1111111111111111111111111111111111111"},{"lamports":1,"address":"SysvarStakeHistory1111111111111111111111111"},{"lamports":1,"address":"SysvarRecentB1ockHashes11111111111111111111"},{"lamports":1,"address":"SysvarFees111111111111111111111111111111111"},{"lamports":1,"address":"Vote111111111111111111111111111111111111111"}]},"id":1}"#;

		let response: ClientResponse<GetLargestAccountsResponse> =
			serde_json::from_str(raw_json).unwrap();
		let expected = ClientResponse {
			jsonrpc: String::from("2.0"),
			result: GetLargestAccountsResponse {
				context: Context { slot: 54 },
				value: vec![
					RpcAccountBalance {
						address: pubkey!("99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ"),
						lamports: 999_974,
					},
					RpcAccountBalance {
						address: pubkey!("uPwWLo16MVehpyWqsLkK3Ka8nLowWvAHbBChqv2FZeL"),
						lamports: 42,
					},
					RpcAccountBalance {
						address: pubkey!("aYJCgU7REfu3XF8b3QhkqgqQvLizx8zxuLBHA25PzDS"),
						lamports: 42,
					},
					RpcAccountBalance {
						address: pubkey!("CTvHVtQ4gd4gUcw3bdVgZJJqApXE9nCbbbP4VTS5wE1D"),
						lamports: 42,
					},
					RpcAccountBalance {
						address: pubkey!("4fq3xJ6kfrh9RkJQsmVd5gNMvJbuSHfErywvEjNQDPxu"),
						lamports: 20,
					},
					RpcAccountBalance {
						address: pubkey!("AXJADheGVp9cruP8WYu46oNkRbeASngN5fPCMVGQqNHa"),
						lamports: 4,
					},
					RpcAccountBalance {
						address: pubkey!("8NT8yS6LiwNprgW4yM1jPPow7CwRUotddBVkrkWgYp24"),
						lamports: 2,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarEpochSchedu1e111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("11111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("Stake11111111111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarC1ock11111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("StakeConfig11111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarRent111111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("Config1111111111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarStakeHistory1111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarRecentB1ockHashes11111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("SysvarFees111111111111111111111111111111111"),
						lamports: 1,
					},
					RpcAccountBalance {
						address: pubkey!("Vote111111111111111111111111111111111111111"),
						lamports: 1,
					},
				],
			},
			id: 1,
		};

		check!(response == expected);
	}
}
