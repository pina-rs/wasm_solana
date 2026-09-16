use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::ser::SerializeTuple;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_error::TransactionError;

use super::Context;
use crate::deserialize_and_decode;
use crate::impl_http_method;
use crate::rpc_config::RpcSimulateTransactionConfig;
use crate::rpc_config::serialize_and_encode;
use crate::solana_account_decoder::UiAccount;
use crate::solana_transaction_status::UiTransactionEncoding;
use crate::solana_transaction_status::UiTransactionReturnData;

/// Request for the `simulateTransaction` RPC method, which dry-runs a signed
/// transaction against the current bank without submitting it.
///
/// The transaction serializes with the version dependent `wincode` wire format,
/// so legacy and v0 transactions place a `short_vec` signature count first
/// while v1 transactions place the message first behind a `0x81` discriminator
/// with signatures at the tail.
#[derive(Debug, PartialEq, Eq)]
pub struct SimulateTransactionRequest {
	/// The signed transaction to simulate.
	pub transaction: VersionedTransaction,
	/// Config controlling signature verification, blockhash replacement,
	/// account encoding, and the accounts to return.
	pub config: Option<RpcSimulateTransactionConfig>,
}

impl Serialize for SimulateTransactionRequest {
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

		let tuple = if let Some(config) = &self.config {
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

impl<'de> Deserialize<'de> for SimulateTransactionRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(rename = "SimulateTransactionRequest")]
		struct Inner(String, Option<RpcSimulateTransactionConfig>);

		let inner = Inner::deserialize(deserializer)?;
		let encoding = match inner.1 {
			Some(ref config) => config.encoding.unwrap_or(UiTransactionEncoding::Base64),
			None => UiTransactionEncoding::Base64,
		};

		let transaction =
			deserialize_and_decode::<VersionedTransaction>(&inner.0, encoding).unwrap();

		Ok(SimulateTransactionRequest {
			transaction,
			config: inner.1,
		})
	}
}

impl_http_method!(SimulateTransactionRequest, "simulateTransaction");

impl SimulateTransactionRequest {
	/// Creates a request that replaces the recent blockhash and skips signature
	/// verification, so unsigned transactions can be simulated.
	pub fn new(transaction: VersionedTransaction) -> Self {
		Self {
			transaction,
			config: Some(RpcSimulateTransactionConfig {
				encoding: Some(UiTransactionEncoding::Base64),
				replace_recent_blockhash: Some(true),
				..Default::default()
			}),
		}
	}

	/// Creates a request with an explicit simulate config.
	pub fn new_with_config(
		transaction: VersionedTransaction,
		config: RpcSimulateTransactionConfig,
	) -> Self {
		Self {
			transaction,
			config: Some(config),
		}
	}
}

/// Simulation result for a single transaction.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SimulateTransactionResponseValue {
	/// Transaction error, or `None` when the simulation succeeded.
	pub err: Option<TransactionError>,
	/// Program log messages emitted during simulation.
	pub logs: Option<Vec<String>>,
	/// Post-simulation account states, present only when the request asked for
	/// accounts.
	pub accounts: Option<Vec<Option<UiAccount>>>,
	/// Compute units consumed by the simulation.
	pub units_consumed: Option<u64>,
	/// Data returned by the program, in the requested encoding.
	pub return_data: Option<UiTransactionReturnData>,
}

/// Response for the `simulateTransaction` RPC method.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulateTransactionResponse {
	/// The slot that the RPC node used to evaluate the request.
	pub context: Context,
	/// Simulation result for the submitted transaction.
	pub value: SimulateTransactionResponseValue,
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use base64::Engine;
	use base64::prelude::BASE64_STANDARD;
	use solana_pubkey::pubkey;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_transaction_status::UiReturnDataEncoding;

	#[test]
	fn request() {
		let tx = wincode::deserialize(&BASE64_STANDARD.decode("AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAEDArczbMia1tLmq7zz4DinMNN0pJ1JtLdqIJPUw3YrGCzYAMHBsgN27lcgB6H2WQvFgyZuJYHa46puOQo9yQ8CVQbd9uHXZaGT2cvhRs7reawctIXtX1s3kTqM9YV+/wCp20C7Wj2aiuk5TReAXo+VTVg8QTHjs0UjNMMKCvpzZ+ABAgEBARU=").unwrap()).unwrap();
		let request = ClientRequest::builder()
			.method(SimulateTransactionRequest::NAME)
			.id(1)
			.params(SimulateTransactionRequest::new_with_config(
				tx,
				RpcSimulateTransactionConfig {
					encoding: Some(UiTransactionEncoding::Base64),
					..Default::default()
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "simulateTransaction",
    "params": [
      "AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAEDArczbMia1tLmq7zz4DinMNN0pJ1JtLdqIJPUw3YrGCzYAMHBsgN27lcgB6H2WQvFgyZuJYHa46puOQo9yQ8CVQbd9uHXZaGT2cvhRs7reawctIXtX1s3kTqM9YV+/wCp20C7Wj2aiuk5TReAXo+VTVg8QTHjs0UjNMMKCvpzZ+ABAgEBARU=",
      {
        "encoding": "base64",
        "sigVerify": false
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"context":{"slot":218},"value":{"err":null,"accounts":null,"logs":["Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri invoke [1]","Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri consumed 2366 of 1400000 compute units","Program return: 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri KgAAAAAAAAA=","Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri success"],"returnData":{"data":["Kg==","base64"],"programId":"83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri"},"unitsConsumed":2366}},"id":1}"#;

		let response: ClientResponse<SimulateTransactionResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(response.result.context.slot == 218);
		check!(
			response.result.value
				== SimulateTransactionResponseValue {
					accounts: None,
					err: None,
					logs: Some(vec![
						"Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri invoke [1]"
							.to_string(),
						"Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri consumed 2366 of \
						 1400000 compute units"
							.to_string(),
						"Program return: 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri KgAAAAAAAAA="
							.to_string(),
						"Program 83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri success".to_string()
					]),
					return_data: Some(UiTransactionReturnData {
						program_id: pubkey!("83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri")
							.to_string(),
						data: ("Kg==".to_string(), UiReturnDataEncoding::Base64)
					}),
					units_consumed: Some(2366)
				}
		);
	}
}
