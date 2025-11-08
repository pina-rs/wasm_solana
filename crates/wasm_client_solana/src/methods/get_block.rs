use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::skip_serializing_none;
use solana_clock::Slot;

use crate::impl_http_method;
use crate::rpc_config::RpcBlockConfig;
use crate::solana_transaction_status::UiConfirmedBlock;

#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct GetBlockRequest {
	pub slot: Slot,
	pub config: Option<RpcBlockConfig>,
}

impl_http_method!(GetBlockRequest, "getBlock");

impl GetBlockRequest {
	pub fn new(slot: Slot) -> Self {
		Self { slot, config: None }
	}

	pub fn new_with_config(slot: Slot, config: RpcBlockConfig) -> Self {
		Self {
			slot,
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GetBlockResponse(UiConfirmedBlock);

impl From<GetBlockResponse> for UiConfirmedBlock {
	fn from(value: GetBlockResponse) -> Self {
		value.0
	}
}

#[cfg(test)]
mod tests {
	use assert2::check;
	use solana_message::MessageHeader;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_transaction_status::EncodedTransaction;
	use crate::solana_transaction_status::EncodedTransactionWithStatusMeta;
	use crate::solana_transaction_status::TransactionDetails;
	use crate::solana_transaction_status::UiCompiledInstruction;
	use crate::solana_transaction_status::UiMessage;
	use crate::solana_transaction_status::UiRawMessage;
	use crate::solana_transaction_status::UiTransaction;
	use crate::solana_transaction_status::UiTransactionEncoding;
	use crate::solana_transaction_status::UiTransactionStatusMeta;

	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetBlockRequest::NAME)
			.id(1)
			.params(GetBlockRequest::new_with_config(
				430,
				RpcBlockConfig {
					encoding: Some(UiTransactionEncoding::Json),
					max_supported_transaction_version: Some(0),
					rewards: Some(false),
					transaction_details: Some(TransactionDetails::Full),
					commitment: None,
				},
			))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getBlock",
    "params": [
      430,
      {
        "encoding": "json",
        "maxSupportedTransactionVersion": 0,
        "rewards": false,
        "transactionDetails": "full"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"blockHeight":428,"blockTime":null,"blockhash":"3Eq21vXNB5s86c62bVuUfTeaMif1N2kUqRPBmGRJhyTA","parentSlot":429,"previousBlockhash":"mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B","transactions":[{"meta":{"err":null,"fee":5000,"innerInstructions":[],"logMessages":[],"postBalances":[499998932500,26858640,1,1,1],"postTokenBalances":[],"preBalances":[499998937500,26858640,1,1,1],"preTokenBalances":[],"rewards":null,"status":{"Ok":null}},"transaction":{"message":{"accountKeys":["3UVYmECPPMZSCqWKfENfuoTv51fTDTWicX9xmBD2euKe","AjozzgE83A3x1sHNUR64hfH7zaEBWeMaFuAN9kQgujrc","SysvarS1otHashes111111111111111111111111111","SysvarC1ock11111111111111111111111111111111","Vote111111111111111111111111111111111111111"],"header":{"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":3,"numRequiredSignatures":1},"instructions":[{"accounts":[1,2,3,0],"data":"37u9WtQpcm6ULa3WRQHmj49EPs4if7o9f1jSRVZpm2dvihR9C8jY4NqEwXUbLwx15HBSNcP1","programIdIndex":4}],"recentBlockhash":"mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B"},"signatures":["2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv"]}}]},"id":1}"#;

		let response: ClientResponse<GetBlockResponse> = serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		let value = response.result.0;

		check!(value.block_height == Some(428));
		check!(value.block_time.is_none());
		check!(
			value.blockhash
				== "3Eq21vXNB5s86c62bVuUfTeaMif1N2kUqRPBmGRJhyTA"
					.parse()
					.unwrap()
		);
		check!(value.parent_slot == 429);
		check!(
			value.previous_blockhash
				== "mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B"
					.parse()
					.unwrap()
		);

		let encoded = EncodedTransactionWithStatusMeta {
			version: None,
			meta: Some(UiTransactionStatusMeta {
					err: None,
					status: Ok(()),
					fee: 5000,
					pre_balances: vec![499_998_937_500, 26_858_640, 1, 1, 1],
					post_balances: vec![499_998_932_500, 26_858_640, 1, 1, 1],
					inner_instructions: Some(vec![]),
					log_messages: Some(vec![]),
					pre_token_balances: Some(vec![]),
					post_token_balances: Some(vec![]),
					rewards: None,
					loaded_addresses: None,
					return_data: None,
					compute_units_consumed: None,
					cost_units: None,

			}),
			transaction: EncodedTransaction::Json(UiTransaction {
					signatures: vec!["2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv".parse().unwrap()],
					message: UiMessage::Raw(UiRawMessage {
							header: MessageHeader {
									num_required_signatures: 1,
									num_readonly_signed_accounts: 0,
									num_readonly_unsigned_accounts: 3
							},
							account_keys: vec!["3UVYmECPPMZSCqWKfENfuoTv51fTDTWicX9xmBD2euKe".parse().unwrap(),
							"AjozzgE83A3x1sHNUR64hfH7zaEBWeMaFuAN9kQgujrc".parse().unwrap(),
							"SysvarS1otHashes111111111111111111111111111".parse().unwrap(),
							"SysvarC1ock11111111111111111111111111111111".parse().unwrap(),
							"Vote111111111111111111111111111111111111111".parse().unwrap()],
							recent_blockhash: "mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B".parse().unwrap(),
							instructions:vec![UiCompiledInstruction {
									data: "37u9WtQpcm6ULa3WRQHmj49EPs4if7o9f1jSRVZpm2dvihR9C8jY4NqEwXUbLwx15HBSNcP1".to_string(),
									accounts: vec![1, 2, 3, 0],
									program_id_index: 4,
									stack_height: None,
							}],
							address_table_lookups: None
					})
			})
		};
		let expected_transactions = Some(vec![encoded]);

		check!(value.transactions == expected_transactions);
	}
}
