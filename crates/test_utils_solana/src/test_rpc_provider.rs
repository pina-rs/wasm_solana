use std::fmt::Display;
use std::sync::Arc;

use async_trait::async_trait;
use derive_more::Deref;
use derive_more::DerefMut;
use futures::future::join_all;
use futures::lock::Mutex;
use send_wrapper::SendWrapper;
use serde_json::Value;
use solana_commitment_config::CommitmentConfig;
use solana_program_test::ProgramTestContext;
use solana_signature::Signature;
use wasm_client_solana::ClientError;
use wasm_client_solana::ClientResponse;
use wasm_client_solana::ClientResult;
use wasm_client_solana::Context;
use wasm_client_solana::GetAccountInfoRequest;
use wasm_client_solana::GetAccountInfoResponse;
use wasm_client_solana::GetBalanceRequest;
use wasm_client_solana::GetBalanceResponse;
use wasm_client_solana::GetLatestBlockhashRequest;
use wasm_client_solana::GetLatestBlockhashResponse;
use wasm_client_solana::GetMultipleAccountsRequest;
use wasm_client_solana::GetMultipleAccountsResponse;
use wasm_client_solana::GetSignatureStatusesRequest;
use wasm_client_solana::GetSignatureStatusesResponse;
use wasm_client_solana::HttpMethod;
use wasm_client_solana::LOCALNET;
use wasm_client_solana::RequestAirdropRequest;
use wasm_client_solana::RequestAirdropResponse;
use wasm_client_solana::RpcProvider;
use wasm_client_solana::SendTransactionRequest;
use wasm_client_solana::SendTransactionResponse;
use wasm_client_solana::SimulateTransactionRequest;
use wasm_client_solana::SimulateTransactionResponse;
use wasm_client_solana::SimulateTransactionResponseValue;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::rpc_response::RpcBlockhash;
use wasm_client_solana::solana_account_decoder::UiAccountEncoding;
use wasm_client_solana::solana_account_decoder::encode_ui_account;
use wasm_client_solana::solana_transaction_status::TransactionConfirmationStatus;
use wasm_client_solana::solana_transaction_status::TransactionStatus;

use crate::ProgramTestContextExtension;

/// An [`RpcProvider`] implementation backed by a [`ProgramTestContext`].
///
/// Lets client code built against a real RPC endpoint run against an in-process
/// banks client during tests. Create a client with
/// [`TestRpcProvider::to_rpc_client`].
///
/// Only a subset of the JSON-RPC surface is implemented. These methods are
/// served: `getAccountInfo`, `getBalance`, `getLatestBlockhash`,
/// `getSignatureStatuses`, `getMultipleAccounts`, `requestAirdrop`,
/// `sendTransaction` and `simulateTransaction`.
///
/// # Panics
///
/// Any other method panics with `todo!`, even though `send` returns
/// [`ClientResult`]. Keep test client usage within the supported set above.
#[derive(Clone, Deref, DerefMut)]
pub struct TestRpcProvider(pub Arc<Mutex<ProgramTestContext>>);

impl TestRpcProvider {
	/// Create a new [`TestRpcProvider`] from the [`ProgramTestContext`].
	pub fn new(ctx: ProgramTestContext) -> Self {
		ctx.into()
	}

	/// Get the wrapped inner [`ProgramTestContext`].
	pub fn inner(&self) -> Arc<Mutex<ProgramTestContext>> {
		self.0.clone()
	}

	/// Wrap the current `RpcProvider` in an `Arc` struct.
	pub fn arc(&self) -> Arc<Self> {
		Arc::new(self.clone())
	}

	/// Create a new instance of the [`SolanaRpcClient`].
	pub fn to_rpc_client(&self) -> SolanaRpcClient {
		SolanaRpcClient::new_with_provider(self.arc(), CommitmentConfig::finalized())
	}
}

impl From<ProgramTestContext> for TestRpcProvider {
	fn from(value: ProgramTestContext) -> Self {
		Self(Arc::new(Mutex::new(value)))
	}
}

#[async_trait]
impl RpcProvider for TestRpcProvider {
	fn url(&self) -> String {
		LOCALNET.to_string()
	}

	async fn send(&self, method: &'static str, request: Value) -> ClientResult<Value> {
		let future = async move {
			let context = {
				Context {
					slot: self.0.lock().await.get_slot().await.map_err(to_error)?,
				}
			};

			let result = match method {
				GetAccountInfoRequest::NAME => {
					let client = self.0.lock().await;
					let request: GetAccountInfoRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let account = client
						.banks_client
						.get_account(request.pubkey)
						.await
						.map_err(to_error)?;
					let encoding = request.config.encoding.unwrap_or(UiAccountEncoding::Base64);

					let data_slice = request.config.data_slice;
					let value = account.map(|account| {
						encoded_account(&request.pubkey, &account, encoding, data_slice)
					});
					let result = GetAccountInfoResponse { context, value };
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				GetBalanceRequest::NAME => {
					let client = self.0.lock().await;
					let request: GetBalanceRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let value = client
						.banks_client
						.get_balance(request.pubkey)
						.await
						.map_err(to_error)?;
					let result = GetBalanceResponse { context, value };
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				GetLatestBlockhashRequest::NAME => {
					let client = self.0.lock().await;
					let blockhash = client
						.banks_client
						.get_latest_blockhash()
						.await
						.map_err(to_error)?;
					let last_valid_block_height = client
						.banks_client
						.get_root_block_height()
						.await
						.map_err(to_error)?;
					let value = RpcBlockhash {
						blockhash,
						last_valid_block_height,
					};
					let result = GetLatestBlockhashResponse { context, value };
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				GetSignatureStatusesRequest::NAME => {
					let client = self.0.lock().await;
					let request: GetSignatureStatusesRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let statuses = client
						.banks_client
						.get_transaction_statuses(request.signatures)
						.await
						.map_err(to_error)?;

					let value = statuses.into_iter().map(convert_status).collect::<Vec<_>>();
					let result = GetSignatureStatusesResponse { context, value };
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				GetMultipleAccountsRequest::NAME => {
					let request: GetMultipleAccountsRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let encoding = request
						.config
						.as_ref()
						.and_then(|config| config.encoding)
						.unwrap_or(UiAccountEncoding::Base64);
					let data_slice = request.config.as_ref().and_then(|config| config.data_slice);
					let futures = request
						.addresses
						.iter()
						.map(|pubkey| fetch_account(self.0.clone(), *pubkey, encoding, data_slice));
					let value = join_all(futures).await;
					let result = GetMultipleAccountsResponse { context, value };
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				// GetTransactionRequest::NAME => {
				// 	let mut client = self.0.lock().await;
				// 	let request: GetTransactionRequest =
				// 		serde_json::from_value(request).map_err(to_error)?;
				// 	let Some(transaction_status) = client
				// 		.banks_client
				// 		.get_transaction_status(request.signature)
				// 		.await
				// 		.map_err(to_error)?
				// 	else {
				// 		return Err(RpcError::default().into());
				// 	};
				// 	let response = ClientResponse {
				// 		jsonrpc: "2.0".into(),
				// 		id: 0,
				// 		result,
				// 	};
				// 	serde_json::to_value(response).map_err(to_error)?
				// }
				RequestAirdropRequest::NAME => {
					let mut client = self.0.lock().await;
					let request: RequestAirdropRequest =
						serde_json::from_value(request).map_err(to_error)?;
					client
						.fund_account(&request.pubkey, request.lamports)
						.await
						.map_err(to_error)?;
					let result = RequestAirdropResponse(Signature::default());
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				SendTransactionRequest::NAME => {
					let client = self.0.lock().await;
					let request: SendTransactionRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let signature = request
						.transaction
						.signatures
						.first()
						.copied()
						.unwrap_or(Signature::default());
					client
						.banks_client
						.send_transaction(request.transaction)
						.await
						.map_err(to_error)?;
					let result: SendTransactionResponse = SendTransactionResponse(signature);
					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				SimulateTransactionRequest::NAME => {
					let client = self.0.lock().await;
					let request: SimulateTransactionRequest =
						serde_json::from_value(request).map_err(to_error)?;
					let transaction = request.transaction.clone();
					let simulation =
						match client.banks_client.simulate_transaction(transaction).await {
							Ok(result) => result,
							Err(_error) => {
								simulate_with_fresh_blockhash(&client.banks_client, request)
									.await
									.map_err(to_error)?
							}
						};

					let result = simulated_response(context, simulation);

					let response = ClientResponse {
						jsonrpc: "2.0".into(),
						id: 0,
						result,
					};

					serde_json::to_value(response).map_err(to_error)?
				}
				value => {
					todo!("This method: `{value}` is not yet implementd for the `TestRpcProvider`.")
				}
			};

			Ok::<Value, ClientError>(result)
		};

		SendWrapper::new(future).await
	}
}

/// Fetch and encode one account for `getMultipleAccounts`; a file-level
/// helper keeps the provider match arms within monostyle's indentation
/// budget.
async fn fetch_account(
	client: Arc<Mutex<ProgramTestContext>>,
	pubkey: solana_pubkey::Pubkey,
	encoding: UiAccountEncoding,
	data_slice: Option<u64>,
) -> Option<crate::solana_account_decoder::UiAccount> {
	let client = client.lock().await;
	let account = client.banks_client.get_account(pubkey).await.unwrap();

	account.map(|account| encoded_account(&pubkey, &account, encoding, data_slice))
}

/// Encode one fetched account for the RPC wire shape; a file-level helper
/// keeps the provider match arms within monostyle's indentation budget.
fn encoded_account(
	pubkey: &solana_pubkey::Pubkey,
	account: &solana_account::AccountSharedData,
	encoding: UiAccountEncoding,
	data_slice: Option<u64>,
) -> crate::solana_account_decoder::UiAccount {
	encode_ui_account(pubkey, account, encoding, None, data_slice)
}

/// Shape one simulation result for the RPC wire; a file-level helper keeps
/// the provider match arms within monostyle's indentation budget.
fn simulated_response(
	context: Context,
	simulation: solana_banks_interface::SimulateTransactionResult,
) -> SimulateTransactionResponse {
	let details = simulation.simulation_details.as_ref();
	SimulateTransactionResponse {
		context,
		value: SimulateTransactionResponseValue {
			err: simulation.result.and_then(|value| value.err().clone()),
			logs: details.map(|v| v.logs.clone()),
			accounts: Some(vec![]),
			units_consumed: details.map(|v| v.units_consumed),
			return_data: details.and_then(|v| v.return_data.clone()),
		},
	}
}

/// Retry a simulation after refreshing the message blockhash: banks-client
/// simulations fail on a stale hash, so the fallback re-signs against the
/// bank's current one.
async fn simulate_with_fresh_blockhash(
	banks_client: &BanksClient,
	request: &SimulateTransactionRequest,
) -> Result<solana_banks_interface::SimulateTransactionResult, ClientError> {
	let mut transaction = request.transaction.clone();
	let blockhash = banks_client
		.get_latest_blockhash()
		.await
		.map_err(to_error)?;
	transaction.message.set_recent_blockhash(blockhash);

	banks_client
		.simulate_transaction(transaction)
		.await
		.map_err(to_error)
}

/// Adapt one banks-client status entry to the RPC wire shape; a file-level
/// helper keeps the provider match arms within monostyle's indentation
/// budget.
fn convert_status(
	maybe_status: Option<solana_banks_interface::TransactionStatus>,
) -> Option<TransactionStatus> {
	maybe_status.map(|status| {
		TransactionStatus {
			slot: status.slot,
			confirmations: status.confirmations,
			status: Ok(()), // legacy field
			err: status.err,
			confirmation_status: status.confirmation_status.map(|v| {
				match v {
					solana_banks_interface::TransactionConfirmationStatus::Processed => {
						TransactionConfirmationStatus::Processed
					}
					solana_banks_interface::TransactionConfirmationStatus::Confirmed => {
						TransactionConfirmationStatus::Confirmed
					}
					solana_banks_interface::TransactionConfirmationStatus::Finalized => {
						TransactionConfirmationStatus::Finalized
					}
				}
			}),
		}
	})
}

fn to_error<T: Display>(error: T) -> ClientError {
	ClientError::Other(error.to_string())
}
