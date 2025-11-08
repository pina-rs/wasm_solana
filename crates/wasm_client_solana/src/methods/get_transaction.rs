use serde::Deserialize;
use serde::Serialize;
use serde_tuple::Deserialize_tuple;
use serde_tuple::Serialize_tuple;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_signature::Signature;

use crate::impl_http_method;
use crate::rpc_config::RpcTransactionConfig;
use crate::solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct GetTransactionRequest {
	#[serde_as(as = "DisplayFromStr")]
	pub signature: Signature,
	pub config: Option<RpcTransactionConfig>,
}

impl_http_method!(GetTransactionRequest, "getTransaction");

impl GetTransactionRequest {
	pub fn new(signature: Signature) -> Self {
		Self {
			signature,
			config: None,
		}
	}

	pub fn new_with_config(signature: Signature, config: RpcTransactionConfig) -> Self {
		Self {
			signature,
			config: Some(config),
		}
	}
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GetTransactionResponse(Option<EncodedConfirmedTransactionWithStatusMeta>);

impl From<GetTransactionResponse> for Option<EncodedConfirmedTransactionWithStatusMeta> {
	fn from(value: GetTransactionResponse) -> Self {
		value.0
	}
}

#[cfg(test)]
mod tests {
	use std::str::FromStr;

	use assert2::check;
	use solana_message::MessageHeader;

	use super::*;
	use crate::ClientRequest;
	use crate::ClientResponse;
	use crate::methods::HttpMethod;
	use crate::solana_transaction_status::EncodedTransaction;
	use crate::solana_transaction_status::UiCompiledInstruction;
	use crate::solana_transaction_status::UiMessage;
	use crate::solana_transaction_status::UiRawMessage;
	use crate::solana_transaction_status::UiTransaction;
	use crate::solana_transaction_status::UiTransactionEncoding;
	use crate::solana_transaction_status::UiTransactionStatusMeta;

	// Serialization differs a bit from the RPC API but it is allowed too
	#[test]
	fn request() {
		let request = ClientRequest::builder()
			.method(GetTransactionRequest::NAME)
			.id(1)
			.params(GetTransactionRequest::new_with_config(Signature::from_str("2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv").unwrap(), RpcTransactionConfig {
					encoding: Some(UiTransactionEncoding::Json),
					..Default::default()
			}))
			.build();

		insta::assert_compact_json_snapshot!(request, @r###"
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getTransaction",
    "params": [
      "2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv",
      {
        "encoding": "json"
      }
    ]
  }
  "###);
	}

	#[test]
	fn response() {
		let raw_json = r#"{"jsonrpc":"2.0","result":{"meta":{"err":null,"fee":5000,"innerInstructions":[],"postBalances":[499998932500,26858640,1,1,1],"postTokenBalances":[],"preBalances":[499998937500,26858640,1,1,1],"preTokenBalances":[],"rewards":[],"status":{"Ok":null}},"slot":430,"transaction":{"message":{"accountKeys":["3UVYmECPPMZSCqWKfENfuoTv51fTDTWicX9xmBD2euKe","AjozzgE83A3x1sHNUR64hfH7zaEBWeMaFuAN9kQgujrc","SysvarS1otHashes111111111111111111111111111","SysvarC1ock11111111111111111111111111111111","Vote111111111111111111111111111111111111111"],"header":{"numReadonlySignedAccounts":0,"numReadonlyUnsignedAccounts":3,"numRequiredSignatures":1},"instructions":[{"accounts":[1,2,3,0],"data":"37u9WtQpcm6ULa3WRQHmj49EPs4if7o9f1jSRVZpm2dvihR9C8jY4NqEwXUbLwx15HBSNcP1","programIdIndex":4}],"recentBlockhash":"mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B"},"signatures":["2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv"]}},"blockTime":null,"id":1}"#;

		let response: ClientResponse<GetTransactionResponse> =
			serde_json::from_str(raw_json).unwrap();

		check!(response.id == 1);
		check!(response.jsonrpc == "2.0");
		check!(
            response.result.0 ==
            Some(EncodedConfirmedTransactionWithStatusMeta {
                block_time: None,
                slot: 430,
                transaction:
                    crate::solana_transaction_status::EncodedTransactionWithStatusMeta {
                        transaction: EncodedTransaction::Json(UiTransaction {
                            message: UiMessage::Raw(UiRawMessage {
                                header: MessageHeader {
                                    num_readonly_signed_accounts: 0,
                                    num_readonly_unsigned_accounts: 3,
                                    num_required_signatures: 1
                                },
                                address_table_lookups: None,
                                recent_blockhash: "mfcyqEXB3DnHXki6KjjmZck6YjmZLvpAByy2fj4nh6B".parse().unwrap(),
                                instructions: vec![UiCompiledInstruction {
                                    program_id_index: 4,
                                    accounts: vec![1, 2, 3, 0],
                                    data: "37u9WtQpcm6ULa3WRQHmj49EPs4if7o9f1jSRVZpm2dvihR9C8jY4NqEwXUbLwx15HBSNcP1".to_string(),
																		stack_height: None,
                                }],
                                account_keys: vec![
                                    "3UVYmECPPMZSCqWKfENfuoTv51fTDTWicX9xmBD2euKe".parse().unwrap(),
                                    "AjozzgE83A3x1sHNUR64hfH7zaEBWeMaFuAN9kQgujrc".parse().unwrap(),
                                    "SysvarS1otHashes111111111111111111111111111".parse().unwrap(),
                                    "SysvarC1ock11111111111111111111111111111111".parse().unwrap(),
                                    "Vote111111111111111111111111111111111111111".parse().unwrap(),
                                ]
                            }),
                            signatures: vec!["2nBhEBYYvfaAe16UMNqRHre4YNSskvuYgx3M6E4JP1oDYvZEJHvoPzyUidNgNX5r9sTyN1J9UxtbCXy2rqYcuyuv".parse().unwrap()]
                        }),
                        version: None,
                        meta: Some(UiTransactionStatusMeta {
                            err: None,
                            fee: 5000,
                            inner_instructions: Some(vec![]),
                            post_balances: vec![499_998_932_500, 26_858_640, 1, 1, 1],
                            post_token_balances: Some(vec![]),
                            pre_balances: vec![499_998_937_500, 26_858_640, 1, 1, 1],
                            pre_token_balances: Some(vec![]),
                            rewards: Some(vec![]),
                            status: Ok(()),
                            loaded_addresses: None,
                            log_messages: None,
                            return_data: None,
														compute_units_consumed: None,
														cost_units: None,
                        }),
                    }
            })
        );
	}
}
