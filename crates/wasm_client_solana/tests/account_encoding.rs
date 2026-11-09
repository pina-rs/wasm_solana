//! Account fetches must request a binary encoding when the caller leaves it
//! unset. A node otherwise answers with `jsonParsed`, and `to_account`
//! turns that payload into `None`, so a live account looks missing.
#![cfg(all(feature = "ssr", not(target_arch = "wasm32")))]

use std::sync::Arc;
use std::sync::Mutex;

use assert2::check;
use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_pubkey::Pubkey;
use solana_pubkey::pubkey;
use wasm_client_solana::ClientResult;
use wasm_client_solana::RpcAccountInfoConfig;
use wasm_client_solana::RpcProgramAccountsConfig;
use wasm_client_solana::RpcProvider;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::solana_account_decoder::UiAccountEncoding;
use wasm_client_solana::solana_account_decoder::UiDataSliceConfig;

const HELLO: &str = "hello world";
const HELLO_BASE64: &str = "aGVsbG8gd29ybGQ=";
const HELLO_BASE58: &str = "StV1DL6CwTryKyV";

struct RecordingProvider {
	requests: Mutex<Vec<Value>>,
}

#[async_trait]
impl RpcProvider for RecordingProvider {
	fn url(&self) -> String {
		"http://127.0.0.1:8899".to_string()
	}

	async fn send(&self, method: &'static str, request: Value) -> ClientResult<Value> {
		self.requests
			.lock()
			.expect("request log")
			.push(json!({ "method": method, "params": request.clone() }));

		Ok(envelope(method, &request))
	}
}

fn client() -> (SolanaRpcClient, Arc<RecordingProvider>) {
	let provider = Arc::new(RecordingProvider {
		requests: Mutex::new(Vec::new()),
	});
	let client =
		SolanaRpcClient::new_with_provider(provider.clone(), CommitmentConfig::confirmed());

	(client, provider)
}

fn encoding_of(params: &Value) -> Option<&str> {
	params
		.get(1)
		.and_then(|config| config.get("encoding"))
		.and_then(Value::as_str)
}

/// Answer the way a node does: binary data for a binary encoding, and a
/// parsed object when the request left encoding unset or asked for
/// `jsonParsed`.
fn envelope(method: &str, params: &Value) -> Value {
	let encoding = encoding_of(params);
	let account = match encoding {
		Some("base64") => binary_account(json!([HELLO_BASE64, "base64"])),
		Some("base58") => binary_account(json!([HELLO_BASE58, "base58"])),
		_ => {
			json!({
				"data": {
					"program": "nonce",
					"parsed": {},
					"space": HELLO.len(),
				},
				"executable": false,
				"lamports": 42,
				"owner": Pubkey::default().to_string(),
				"rentEpoch": 0,
				"space": HELLO.len(),
			})
		}
	};

	let result = match method {
		"getMultipleAccounts" => {
			json!({
				"context": { "slot": 1 },
				"value": [account, Value::Null],
			})
		}
		"getAccountInfo" => {
			json!({
				"context": { "slot": 1 },
				"value": account,
			})
		}
		"getProgramAccounts" => {
			json!([{
				"pubkey": pubkey!("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg").to_string(),
				"account": account,
			}])
		}
		other => panic!("unexpected method {other}"),
	};

	json!({
		"jsonrpc": "2.0",
		"id": 1,
		"result": result,
	})
}

fn binary_account(data: Value) -> Value {
	json!({
		"data": data,
		"executable": false,
		"lamports": 42,
		"owner": Pubkey::default().to_string(),
		"rentEpoch": 0,
		"space": HELLO.len(),
	})
}

fn last_request(provider: &RecordingProvider) -> Value {
	provider
		.requests
		.lock()
		.expect("request log")
		.last()
		.expect("a request was sent")
		.clone()
}

#[tokio::test]
async fn unset_encoding_fetches_base64_and_keeps_alignment() {
	let (client, provider) = client();
	let existing = pubkey!("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg");
	let missing = pubkey!("4fYNw3dojWmQ4dXtSGE9epjRGy9pFSx62YypT7avPYvA");

	let accounts = client
		.get_multiple_accounts_with_config(
			&[existing, missing],
			RpcAccountInfoConfig {
				commitment: Some(CommitmentConfig::confirmed()),
				data_slice: Some(UiDataSliceConfig {
					offset: 1,
					length: 4,
				}),
				..Default::default()
			},
		)
		.await
		.expect("multiple accounts");

	let request = last_request(&provider);
	check!(request["method"] == "getMultipleAccounts");
	check!(encoding_of(&request["params"]) == Some("base64"));
	check!(request["params"][1]["dataSlice"]["offset"] == 1);
	check!(request["params"][1]["dataSlice"]["length"] == 4);

	let found = accounts[0].as_ref().expect("existing account decodes");
	check!(found.lamports == 42);
	check!(found.data == HELLO.as_bytes());
	check!(accounts[1].is_none());
}

#[tokio::test]
async fn explicit_encoding_is_preserved() {
	let (client, provider) = client();
	let pubkey = pubkey!("vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg");

	let account = client
		.get_account_with_config(
			&pubkey,
			RpcAccountInfoConfig {
				encoding: Some(UiAccountEncoding::Base58),
				..Default::default()
			},
		)
		.await
		.expect("account")
		.expect("base58 account decodes");

	let request = last_request(&provider);
	check!(encoding_of(&request["params"]) == Some("base58"));
	check!(account.data == HELLO.as_bytes());

	let parsed = client
		.get_account_with_config(
			&pubkey,
			RpcAccountInfoConfig {
				encoding: Some(UiAccountEncoding::JsonParsed),
				..Default::default()
			},
		)
		.await
		.expect("parsed request succeeds");

	check!(parsed.is_none());
	check!(encoding_of(&last_request(&provider)["params"]) == Some("jsonParsed"));
}

#[tokio::test]
async fn program_accounts_default_to_base64() {
	let (client, provider) = client();
	let program = pubkey!("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T");

	let accounts = client
		.get_program_accounts_with_config(
			&program,
			RpcProgramAccountsConfig {
				account_config: RpcAccountInfoConfig {
					commitment: Some(CommitmentConfig::confirmed()),
					..Default::default()
				},
				..Default::default()
			},
		)
		.await
		.expect("program accounts");

	check!(encoding_of(&last_request(&provider)["params"]) == Some("base64"));
	check!(accounts.len() == 1);
	check!(accounts[0].1.data == HELLO.as_bytes());

	let error = client
		.get_program_accounts_with_config(
			&program,
			RpcProgramAccountsConfig {
				account_config: RpcAccountInfoConfig {
					encoding: Some(UiAccountEncoding::JsonParsed),
					..Default::default()
				},
				..Default::default()
			},
		)
		.await
		.expect_err("jsonParsed program accounts cannot be decoded");

	check!(error.to_string().contains("Unable to decode"));
	check!(encoding_of(&last_request(&provider)["params"]) == Some("jsonParsed"));
}

#[test]
fn default_config_encoding_matches_the_builder() {
	check!(RpcAccountInfoConfig::default().encoding == Some(UiAccountEncoding::Base64));
	check!(RpcAccountInfoConfig::builder().build().encoding == Some(UiAccountEncoding::Base64));
}
