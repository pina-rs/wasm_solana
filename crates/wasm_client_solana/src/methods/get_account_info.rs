use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_pubkey::Pubkey;
use typed_builder::TypedBuilder;

use super::Context;
use crate::impl_http_method;
use crate::impl_websocket_method;
use crate::impl_websocket_notification;
use crate::rpc_config::RpcAccountInfoConfig;
use crate::solana_account_decoder::UiAccount;

/// Use the builder pattern to create a request for account info.
///
/// ```rust
/// use solana_pubkey::pubkey;
/// use wasm_client_solana::GetAccountInfoRequest;
///
/// let request = GetAccountInfoRequest::builder()
/// 	.pubkey(pubkey!("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T"))
/// 	.build();
/// ```
#[derive(Debug, TypedBuilder)]
pub struct GetAccountInfoRequest {
	pub pubkey: Pubkey,
	#[builder(default = RpcAccountInfoConfig::builder().build())]
	pub config: RpcAccountInfoConfig,
}

impl From<Pubkey> for GetAccountInfoRequest {
	fn from(pubkey: Pubkey) -> Self {
		Self::builder().pubkey(pubkey).build()
	}
}

impl From<&Pubkey> for GetAccountInfoRequest {
	fn from(pubkey: &Pubkey) -> Self {
		Self::builder().pubkey(*pubkey).build()
	}
}

impl Serialize for GetAccountInfoRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		#[derive(Serialize)]
		#[serde(rename = "GetAccountInfoRequest")]
		#[derive(::serde_with::__private_consume_serde_as_attributes)]
		struct Inner<'serde_tuple_inner>(
			#[serde_as(as = "DisplayFromStr")]
			#[serde(with = "::serde_with::As::<DisplayFromStr>")]
			&'serde_tuple_inner Pubkey,
			&'serde_tuple_inner RpcAccountInfoConfig,
		);

		let inner = Inner(&self.pubkey, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

impl<'de> Deserialize<'de> for GetAccountInfoRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(rename = "GetAccountInfoRequest")]
		#[derive(::serde_with::__private_consume_serde_as_attributes)]
		struct Inner(
			#[serde_as(as = "DisplayFromStr")]
			#[serde(with = "::serde_with::As::<DisplayFromStr>")]
			Pubkey,
			RpcAccountInfoConfig,
		);

		let inner = Inner::deserialize(deserializer)?;
		Ok(GetAccountInfoRequest::builder()
			.pubkey(inner.0)
			.config(inner.1)
			.build())
	}
}

impl_http_method!(GetAccountInfoRequest, "getAccountInfo");
impl_websocket_method!(GetAccountInfoRequest, "account");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetAccountInfoResponse {
	pub context: Context,
	pub value: Option<UiAccount>,
}

impl_websocket_notification!(GetAccountInfoResponse, "account");

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
		let pubkey = pubkey!("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg");
		let request = ClientRequest::builder()
			.method(GetAccountInfoRequest::NAME)
			.id(1)
			.params(GetAccountInfoRequest::from(pubkey))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getAccountInfo",
    "params": [
      "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg",
      {
        "encoding": "base64"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":1},"value":{"data":["11116bv5nS2h3y12kD1yUKeMZvGcKLSjQgX6BeV7u1FrjeJcKfsHRTPuR3oZ1EioKtYGiYxpxMG5vpbZLsbcBYBEmZZcMKaSoGx9JZeAuWf","base58"],"executable":false,"lamports":1000000000,"owner":"11111111111111111111111111111111","rentEpoch":2,"space":80}},"id":1}"#;

		let response: ClientResponse<GetAccountInfoResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 1);

		let value = response.result.value.unwrap();
		assert!(!value.executable);
		check!(value.lamports == 1_000_000_000);
		check!(value.owner == Pubkey::default());
		check!(value.rent_epoch == 2);
		check!(value.space == Some(80));
		check!(value.data == UiAccountData::Binary("11116bv5nS2h3y12kD1yUKeMZvGcKLSjQgX6BeV7u1FrjeJcKfsHRTPuR3oZ1EioKtYGiYxpxMG5vpbZLsbcBYBEmZZcMKaSoGx9JZeAuWf".to_string(), UiAccountEncoding::Base58));
	}
}
