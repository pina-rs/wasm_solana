use serde::Deserialize;
use serde::Serialize;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_pubkey::Pubkey;

use crate::impl_http_method;

#[derive(Debug, Serialize)]
pub struct GetIdentityRequest;

impl_http_method!(GetIdentityRequest, "getIdentity");

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetIdentityResponse {
	#[serde_as(as = "DisplayFromStr")]
	pub identity: Pubkey,
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
			.method(GetIdentityRequest::NAME)
			.id(1)
			.params(GetIdentityRequest)
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getIdentity"}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"identity":"2r1F4iWqVcb8M1DbAjQuFpebkQHY9hcVU4WuW2DJBppN"},"id":1}"#;

		let response: ClientResponse<GetIdentityResponse> = serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.identity == pubkey!("2r1F4iWqVcb8M1DbAjQuFpebkQHY9hcVU4WuW2DJBppN"));
	}
}
