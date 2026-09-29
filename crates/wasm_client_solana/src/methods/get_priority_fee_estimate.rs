use derive_more::derive::From;
use derive_more::derive::Into;
use serde::Deserialize;
use serde::Serialize;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use solana_pubkey::Pubkey;
use solana_transaction::versioned::VersionedTransaction;
use typed_builder::TypedBuilder;

use crate::impl_http_method;
use crate::rpc_config::serialize_and_encode;
use crate::solana_transaction_status::UiTransactionEncoding;

/// How aggressively to price a transaction, as a percentile of recent network
/// activity. `Min` lands at the bottom of what still lands; `UnsafeMax` chases
/// the very top of the fee market and can be expensive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcPriorityFeeLevel {
	/// The minimum fee that recently landed — cheapest, weakest inclusion
	/// odds.
	Min,
	/// The 25th percentile of recent landing fees.
	Low,
	/// The 50th percentile; a balanced default for interactive sends.
	Medium,
	/// The 75th percentile; for transactions that must land promptly.
	High,
	/// The 90th percentile; for arbitrage-style latency sensitivity.
	VeryHigh,
	/// The maximum observed fee — expensive, included only for completeness.
	UnsafeMax,
	/// Let the node pick its default level.
	#[serde(other)]
	Default,
}

/// Config for [`GetPriorityFeeEstimateRequest`]. Exactly one of `recommended`
/// or `priority_level` should steer the estimate; when neither is set the node
/// answers with its default level.
#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcPriorityFeeEstimateConfig {
	/// Ask for a single estimate the node considers reasonable right now.
	pub recommended: Option<bool>,
	/// Ask for a specific percentile of the recent fee market instead.
	pub priority_level: Option<RpcPriorityFeeLevel>,
	/// Encoding of the `transaction` field; `base64` is the only value nodes
	/// accept today.
	pub transaction_encoding: Option<UiTransactionEncoding>,
	/// How many recent slots of fee activity to consider. More slots smooth
	/// the estimate; fewer react faster to congestion spikes.
	pub lookback_slots: Option<u8>,
}

/// Request for the `getPriorityFeeEstimate` RPC method: the estimated
/// prioritization fee (in micro-lamports per compute unit) a transaction
/// should carry to land at the chosen urgency.
///
/// Provide a full `transaction` for the most accurate estimate — the node
/// simulates its actual compute-unit usage — or just `account_keys` for a
/// cheaper estimate based on the fee activity of the accounts a transaction
/// will lock.
#[serde_as]
#[derive(Debug)]
pub struct GetPriorityFeeEstimateRequest {
	transaction: Option<String>,
	account_keys: Vec<Pubkey>,
	options: Option<RpcPriorityFeeEstimateConfig>,
}

/// The method takes a single object parameter, unlike the positional arrays
/// most `get*` methods use, so serialization is written by hand: one
/// camelCase, `skip_serializing_if` object wrapped in a one-element params
/// array.
impl Serialize for GetPriorityFeeEstimateRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		#[serde_as]
		#[derive(Serialize)]
		#[serde(rename_all = "camelCase")]
		struct Params<'a> {
			#[serde(skip_serializing_if = "Option::is_none")]
			transaction: &'a Option<String>,
			#[serde(skip_serializing_if = "Vec::is_empty")]
			#[serde_as(as = "Vec<DisplayFromStr>")]
			account_keys: &'a Vec<Pubkey>,
			#[serde(skip_serializing_if = "Option::is_none")]
			options: &'a Option<RpcPriorityFeeEstimateConfig>,
		}

		let params = Params {
			transaction: &self.transaction,
			account_keys: &self.account_keys,
			options: &self.options,
		};
		let value = serde_json::to_value(params).map_err(serde::ser::Error::custom)?;
		[value].serialize(serializer)
	}
}

impl_http_method!(GetPriorityFeeEstimateRequest, "getPriorityFeeEstimate");

impl GetPriorityFeeEstimateRequest {
	/// Estimate from a transaction the caller has already signed (or is
	/// about to sign); the node simulates its actual compute-unit usage, so
	/// this is the most accurate form.
	pub fn new(transaction: &VersionedTransaction) -> crate::ClientResult<Self> {
		Ok(Self {
			transaction: Some(serialize_and_encode(
				transaction,
				UiTransactionEncoding::Base64,
			)?),
			account_keys: Vec::new(),
			options: None,
		})
	}

	/// Estimate from the accounts a transaction will lock. Cheaper for the
	/// node than a full transaction, less accurate for transactions with
	/// unusual compute-unit usage.
	pub fn new_for_accounts(account_keys: Vec<Pubkey>) -> Self {
		Self {
			transaction: None,
			account_keys,
			options: None,
		}
	}

	/// Set the estimate options (recommended, level, encoding, lookback).
	#[must_use]
	pub fn with_options(mut self, config: RpcPriorityFeeEstimateConfig) -> Self {
		self.options = Some(config);
		self
	}
}

/// Response for the `getPriorityFeeEstimate` RPC method.
///
/// `None` means the node could not produce an estimate (for example no recent
/// fee activity for the given accounts); treat it as "unknown" rather than
/// "free".
#[derive(Debug, Deserialize, From, Into)]
#[serde(rename_all = "camelCase")]
pub struct GetPriorityFeeEstimateResponse {
	/// The estimated prioritization fee in micro-lamports per compute unit.
	pub priority_fee_estimate: Option<u64>,
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
	fn request_for_accounts() {
		let request = ClientRequest::builder()
			.method(GetPriorityFeeEstimateRequest::NAME)
			.id(1)
			.params(
				GetPriorityFeeEstimateRequest::new_for_accounts(vec![pubkey!(
					"11111111111111111111111111111111"
				)])
				.with_options(
					RpcPriorityFeeEstimateConfig::builder()
						.recommended(true)
						.build(),
				),
			)
			.build();

		insta::assert_compact_json_snapshot!(request, @r#"
		{
		  "jsonrpc": "2.0",
		  "id": 1,
		  "method": "getPriorityFeeEstimate",
		  "params": [
		    {
		      "accountKeys": [
		        "11111111111111111111111111111111"
		      ],
		      "options": {
		        "recommended": true
		      }
		    }
		  ]
		}
		"#);
	}

	#[test]
	fn response() {
		let raw_json =
			r#"{ "jsonrpc": "2.0", "result": { "priorityFeeEstimate": 120000 }, "id": 1 }"#;

		let response: ClientResponse<GetPriorityFeeEstimateResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.result.priority_fee_estimate == Some(120_000));
	}

	#[test]
	fn response_none() {
		let raw_json =
			r#"{ "jsonrpc": "2.0", "result": { "priorityFeeEstimate": null }, "id": 1 }"#;

		let response: ClientResponse<GetPriorityFeeEstimateResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.result.priority_fee_estimate.is_none());
	}
}
