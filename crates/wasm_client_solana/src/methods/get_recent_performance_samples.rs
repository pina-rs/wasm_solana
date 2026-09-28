use serde::Deserialize;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;

use crate::impl_http_method;
use crate::rpc_response::RpcPerfSample;

/// Request for the `getRecentPerformanceSamples` RPC method, which returns
/// recent performance samples in slots-per-second terms.
#[skip_serializing_none]
#[derive(Debug, Default, Serialize_tuple)]
pub struct GetRecentPerformanceSamplesRequest {
	/// Maximum number of samples to return. Defaults to the node's maximum when
	/// omitted.
	pub limit: Option<usize>,
}

impl_http_method!(
	GetRecentPerformanceSamplesRequest,
	"getRecentPerformanceSamples"
);

impl GetRecentPerformanceSamplesRequest {
	/// Creates a request for the node's default number of samples.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a request for at most `limit` samples.
	pub fn new_with_limit(limit: usize) -> Self {
		Self { limit: Some(limit) }
	}
}

/// Response for the `getRecentPerformanceSamples` RPC method.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct GetRecentPerformanceSamplesResponse(Vec<RpcPerfSample>);

impl From<GetRecentPerformanceSamplesResponse> for Vec<RpcPerfSample> {
	fn from(val: GetRecentPerformanceSamplesResponse) -> Self {
		val.0
	}
}

#[cfg(test)]
mod tests {

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetRecentPerformanceSamplesRequest::NAME)
			.id(1)
			.params(GetRecentPerformanceSamplesRequest::new_with_limit(4))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"{"jsonrpc": "2.0", "id": 1, "method": "getRecentPerformanceSamples", "params": [4]}"###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":[{"numSlots":126,"numTransactions":126,"numNonVoteTransaction":1,"samplePeriodSecs":60,"slot":348125},{"numSlots":126,"numTransactions":126,"numNonVoteTransaction":1,"samplePeriodSecs":60,"slot":347999},{"numSlots":125,"numTransactions":125,"numNonVoteTransaction":0,"samplePeriodSecs":60,"slot":347873},{"numSlots":125,"numTransactions":125,"numNonVoteTransaction":0,"samplePeriodSecs":60,"slot":347748}],"id":1}"#;

		let response: ClientResponse<GetRecentPerformanceSamplesResponse> =
			serde_json::from_str(raw_json).unwrap();

		assert_eq!(response.id, 1);
		assert_eq!(response.jsonrpc, "2.0");
		let value = response.result.0;
		assert_eq!(
			value,
			vec![
				RpcPerfSample {
					num_slots: 126,
					num_transactions: 126,
					slot: 348_125,
					sample_period_secs: 60,
					num_non_vote_transaction: 1
				},
				RpcPerfSample {
					num_slots: 126,
					num_transactions: 126,
					slot: 347_999,
					sample_period_secs: 60,
					num_non_vote_transaction: 1
				},
				RpcPerfSample {
					num_slots: 125,
					num_transactions: 125,
					slot: 347_873,
					sample_period_secs: 60,
					num_non_vote_transaction: 0
				},
				RpcPerfSample {
					num_slots: 125,
					num_transactions: 125,
					slot: 347_748,
					sample_period_secs: 60,
					num_non_vote_transaction: 0
				}
			]
		);
	}
}
