use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::SerializeTuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;

use crate::deserialize_and_decode;
use crate::impl_http_method;
use crate::rpc_config::RpcSendTransactionConfig;
use crate::rpc_config::serialize_and_encode;
use crate::solana_transaction_status::UiTransactionEncoding;

#[derive(Debug, PartialEq, Eq)]
pub struct SendTransactionRequest {
	pub transaction: VersionedTransaction,
	pub config: Option<RpcSendTransactionConfig>,
}

impl Serialize for SendTransactionRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let encoding = match self.config {
			Some(ref c) => c.encoding.unwrap_or(UiTransactionEncoding::Base64),
			None => UiTransactionEncoding::Base64,
		};

		let serialized_encoded =
			serialize_and_encode::<VersionedTransaction>(&self.transaction, encoding).unwrap();

		let tuple = if let Some(config) = self.config {
			let mut tuple = serializer.serialize_tuple(2)?;
			tuple.serialize_element(&serialized_encoded)?;
			tuple.serialize_element(&config)?;
			tuple
		} else {
			let mut tuple = serializer.serialize_tuple(1)?;
			tuple.serialize_element(&serialized_encoded)?;
			tuple
		};

		tuple.end()
	}
}

impl<'de> Deserialize<'de> for SendTransactionRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(rename = "SendTransactionRequest")]
		struct Inner(String, Option<RpcSendTransactionConfig>);

		let inner = Inner::deserialize(deserializer)?;
		let encoding = match inner.1 {
			Some(ref config) => config.encoding.unwrap_or(UiTransactionEncoding::Base64),
			None => UiTransactionEncoding::Base64,
		};
		let transaction =
			deserialize_and_decode::<VersionedTransaction>(&inner.0, encoding).unwrap();

		Ok(SendTransactionRequest {
			transaction,
			config: inner.1,
		})
	}
}

impl_http_method!(SendTransactionRequest, "sendTransaction");

impl SendTransactionRequest {
	pub fn new(transaction: VersionedTransaction) -> Self {
		Self {
			transaction,
			config: None,
		}
	}

	pub fn new_with_config(
		transaction: VersionedTransaction,
		config: RpcSendTransactionConfig,
	) -> Self {
		Self {
			transaction,
			config: Some(config),
		}
	}
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendTransactionResponse(#[serde_as(as = "DisplayFromStr")] pub Signature);

impl From<SendTransactionResponse> for Signature {
	fn from(val: SendTransactionResponse) -> Self {
		val.0
	}
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_transaction::Transaction;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let tx: Transaction = bincode::deserialize(&bs58::decode("4hXTCkRzt9WyecNzV1XPgCDfGAZzQKNxLXgynz5QDuWWPSAZBZSHptvWRL3BjCvzUXRdKvHL2b7yGrRQcWyaqsaBCncVG7BFggS8w9snUts67BSh3EqKpXLUm5UMHfD7ZBe9GhARjbNQMLJ1QD3Spr6oMTBU6EhdB4RD8CP2xUxr2u3d6fos36PD98XS6oX8TQjLpsMwncs5DAMiD4nNnR8NBfyghGCWvCVifVwvA8B8TJxE1aiyiv2L429BCWfyzAme5sZW8rDb14NeCQHhZbtNqfXhcp2tAnaAT").into_vec().unwrap()).unwrap();
		let request = ClientRequest::builder()
			.method(SendTransactionRequest::NAME)
			.id(1)
			.params(SendTransactionRequest::new(tx.into()))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "sendTransaction",
    "params": [
      "AVXo5X7UNzpuOmYzkZ+fqHDGiRLTSMlWlUCcZKzEV5CIKlrdvZa3/2GrJJfPrXgZqJbYDaGiOnP99tI/sRJfiwwBAAEDRQ/n5E5CLbMbHanUG3+iVvBAWZu0WFM6NoB5xfybQ7kNwwgfIhv6odn2qTUu/gOisDtaeCW1qlwW/gx3ccr/4wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAvsInicc+E3IZzLqeA+iM5cn9kSaeFzOuClz1Z2kZQy0BAgIAAQwCAAAAAPIFKgEAAAA="
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":"2id3YC2jK9G5Wo2phDx4gJVAew8DcY5NAojnVuao8rkxwPYPe8cSwE5GzhEgJA2y8fVjDEo6iR6ykBvDxrTQrtpb","id":1}"#;

		let response: ClientResponse<SendTransactionResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.0 == "2id3YC2jK9G5Wo2phDx4gJVAew8DcY5NAojnVuao8rkxwPYPe8cSwE5GzhEgJA2y8fVjDEo6iR6ykBvDxrTQrtpb".parse().unwrap());
	}
}
