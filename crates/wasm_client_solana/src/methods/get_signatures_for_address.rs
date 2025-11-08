use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_pubkey::Pubkey;

use crate::impl_http_method;
use crate::rpc_config::RpcSignaturesForAddressConfig;
use crate::rpc_response::RpcConfirmedTransactionStatusWithSignature;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple)]
pub struct GetSignaturesForAddressRequest {
	#[serde_as(as = "DisplayFromStr")]
	pubkey: Pubkey,
	config: Option<RpcSignaturesForAddressConfig>,
}

impl_http_method!(GetSignaturesForAddressRequest, "getSignaturesForAddress");

impl GetSignaturesForAddressRequest {
	pub fn new(pubkey: Pubkey) -> Self {
		Self {
			pubkey,
			config: None,
		}
	}

	pub fn new_with_config(pubkey: Pubkey, config: RpcSignaturesForAddressConfig) -> Self {
		Self {
			pubkey,
			config: Some(config),
		}
	}
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetSignaturesForAddressResponse(Vec<RpcConfirmedTransactionStatusWithSignature>);

impl From<GetSignaturesForAddressResponse> for Vec<RpcConfirmedTransactionStatusWithSignature> {
	fn from(val: GetSignaturesForAddressResponse) -> Self {
		val.0
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
			.method(GetSignaturesForAddressRequest::NAME)
			.id(1)
			.params(GetSignaturesForAddressRequest::new_with_config(
				pubkey!("Vote111111111111111111111111111111111111111"),
				RpcSignaturesForAddressConfig {
					limit: Some(1),
					..Default::default()
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getSignaturesForAddress",
    "params": [
      "Vote111111111111111111111111111111111111111",
      {
        "limit": 1
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"err":null,"memo":null,"signature":"5h6xBEauJ3PK6SWCZ1PGjBvj8vDdWG3KpwATGy1ARAXFSDwt8GFXM7W5Ncn16wmqokgpiKRLuS83KUxyZyv2sUYv","slot":114,"blockTime":null}],"id":1}"#;

		let response: ClientResponse<GetSignaturesForAddressResponse> =
			serde_json::from_str(raw_json).unwrap();
		let rpc = RpcConfirmedTransactionStatusWithSignature {
			block_time: None,
			err: None,
			memo: None,
			slot: 114,
			signature: "5h6xBEauJ3PK6SWCZ1PGjBvj8vDdWG3KpwATGy1ARAXFSDwt8GFXM7W5Ncn16wmqokgpiKRLuS83KUxyZyv2sUYv".parse().unwrap(),
			confirmation_status: None
		};
		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == vec![rpc]);
	}
}
