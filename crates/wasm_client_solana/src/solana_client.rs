use std::sync::Arc;
use std::time::Duration;

use futures_timer::Delay;
use serde::de::DeserializeOwned;
use solana_commitment_config::CommitmentConfig;
use solana_commitment_config::CommitmentLevel;
use solana_sdk::account::Account;
use solana_sdk::clock::Epoch;
use solana_sdk::clock::Slot;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::epoch_info::EpochInfo;
use solana_sdk::epoch_schedule::EpochSchedule;
use solana_sdk::hash::Hash;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;

use crate::ClientError;
use crate::ClientResponse;
use crate::ClientResult;
use crate::HttpProvider;
use crate::MAX_RETRIES;
use crate::RpcError;
use crate::RpcProvider;
use crate::SLEEP_MS;
use crate::Subscription;
use crate::WebSocketProvider;
use crate::methods::*;
use crate::rpc_config::BlockSubscribeRequest;
use crate::rpc_config::GetConfirmedSignaturesForAddress2Config;
use crate::rpc_config::LogsSubscribeRequest;
use crate::rpc_config::ProgramSubscribeRequest;
use crate::rpc_config::RpcAccountInfoConfig;
use crate::rpc_config::RpcBlockConfig;
use crate::rpc_config::RpcBlockProductionConfig;
use crate::rpc_config::RpcContextConfig;
use crate::rpc_config::RpcEpochConfig;
use crate::rpc_config::RpcGetVoteAccountsConfig;
use crate::rpc_config::RpcKeyedAccount;
use crate::rpc_config::RpcLargestAccountsConfig;
use crate::rpc_config::RpcLeaderScheduleConfig;
use crate::rpc_config::RpcProgramAccountsConfig;
use crate::rpc_config::RpcSendTransactionConfig;
use crate::rpc_config::RpcSignaturesForAddressConfig;
use crate::rpc_config::RpcSimulateTransactionConfig;
use crate::rpc_config::RpcSupplyConfig;
use crate::rpc_config::RpcTokenAccountsFilter;
use crate::rpc_config::RpcTransactionConfig;
use crate::rpc_filter::TokenAccountsFilter;
use crate::rpc_response::BlockNotificationResponse;
use crate::rpc_response::LogsNotificationResponse;
use crate::rpc_response::RpcAccountBalance;
use crate::rpc_response::RpcBlockProduction;
use crate::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use crate::rpc_response::RpcInflationGovernor;
use crate::rpc_response::RpcInflationRate;
use crate::rpc_response::RpcInflationReward;
use crate::rpc_response::RpcLeaderSchedule;
use crate::rpc_response::RpcPerfSample;
use crate::rpc_response::RpcPrioritizationFee;
use crate::rpc_response::RpcSupply;
use crate::rpc_response::RpcVersionInfo;
use crate::rpc_response::RpcVoteAccountStatus;
use crate::solana_account_decoder::UiAccountData;
use crate::solana_account_decoder::UiAccountEncoding;
use crate::solana_account_decoder::parse_address_lookup_table::LookupTableAccountType;
use crate::solana_account_decoder::parse_address_lookup_table::parse_address_lookup_table;
use crate::solana_account_decoder::parse_token::TokenAccountType;
use crate::solana_account_decoder::parse_token::UiTokenAccount;
use crate::solana_account_decoder::parse_token::UiTokenAmount;
use crate::solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;
use crate::solana_transaction_status::TransactionConfirmationStatus;
use crate::solana_transaction_status::TransactionStatus;
use crate::solana_transaction_status::UiConfirmedBlock;
use crate::solana_transaction_status::UiTransactionEncoding;

/// A client of a remote Solana node.
///
/// `RpcClient` communicates with a Solana node over [JSON-RPC], with the
/// [Solana JSON-RPC protocol][jsonprot]. It is the primary Rust interface for
/// querying and transacting with the network from external programs.
///
/// This type builds on the underlying RPC protocol, adding extra features such
/// as timeout handling, retries, and waiting on transaction [commitment
/// levels][cl]. Some methods simply pass through to the underlying RPC
/// protocol. Not all RPC methods are encapsulated by this type, but
/// `SolanaRpcClient` does expose a generic [`send`](SolanaRpcClient::send)
/// method for making any [`ClientRequest`].
///
/// The documentation for most [`SolanaRpcClient`] methods contains an "RPC
/// Reference" section that links to the documentation for the underlying
/// JSON-RPC method. The documentation for `RpcClient` does not reproduce the
/// documentation for the underlying JSON-RPC methods. Thus reading both is
/// necessary for complete understanding.
///
/// `RpcClient`s generally communicate over HTTP on port 8899, a typical server
/// URL being "<http://localhost:8899>".
///
/// Methods that query information from recent [slots], including those that
/// confirm transactions, decide the most recent slot to query based on a
/// [commitment level][cl], which determines how committed or finalized a slot
/// must be to be considered for the query. Unless specified otherwise, the
/// commitment level is [`Finalized`], meaning the slot is definitely
/// permanently committed. The default commitment level can be configured by
/// creating [`SolanaRpcClient`] with an explicit [`CommitmentConfig`], and that
/// default configured commitment level can be overridden by calling the various
/// `_with_commitment` methods, like
/// [`SolanaRpcClient::confirm_transaction_with_commitment`]. In some cases the
/// configured commitment level is ignored and `Finalized` is used instead, as
/// in [`SolanaRpcClient::get_blocks`], where it would be invalid to use the
/// [`Processed`] commitment level. These exceptions are noted in the method
/// documentation.
///
/// [`Finalized`]: CommitmentLevel::Finalized
/// [`Processed`]: CommitmentLevel::Processed
/// [jsonprot]: https://solana.com/docs/rpc
/// [JSON-RPC]: https://www.jsonrpc.org/specification
/// [slots]: https://solana.com/docs/terminology#slot
/// [cl]: https://solana.com/docs/rpc#configuring-state-commitment
///
/// # Errors
///
/// Methods on [`SolanaRpcClient`] return
/// [`ClientResult`], and many of them
/// return [`ClientResponse`].
///
/// Requests may timeout, in which case they return a [`ClientError`].
#[derive(derive_more::Debug, Clone)]
pub struct SolanaRpcClient {
	commitment_config: CommitmentConfig,
	#[debug(skip)]
	provider: Arc<dyn RpcProvider + Send + Sync + 'static>,
	ws: WebSocketProvider,
}

impl<S: Into<String>> From<S> for SolanaRpcClient {
	fn from(value: S) -> Self {
		Self::new(&value.into())
	}
}

impl From<&SolanaRpcClient> for SolanaRpcClient {
	fn from(value: &SolanaRpcClient) -> Self {
		value.clone()
	}
}

impl SolanaRpcClient {
	/// Create an HTTP `SolanaRpcClient`.
	///
	/// The URL is an HTTP URL, usually for port 8899, as in
	/// "<http://localhost:8899>".
	///
	/// The client has a default timeout of 30 seconds, and a default
	/// [commitment level][cl] of [`Finalized`](CommitmentLevel::Finalized).
	///
	/// [cl]: https://solana.com/docs/rpc#configuring-state-commitment
	pub fn new(endpoint: &str) -> Self {
		Self {
			provider: Arc::new(HttpProvider::new(endpoint)),
			commitment_config: CommitmentConfig::confirmed(),
			ws: WebSocketProvider::new(endpoint),
		}
	}

	/// Create an HTTP `RpcClient` with specified [commitment level][cl].
	///
	/// [cl]: https://solana.com/docs/rpc#configuring-state-commitment
	///
	/// The URL is an HTTP URL, usually for port 8899, as in
	/// "<http://localhost:8899>".
	///
	/// The client has a default timeout of 30 seconds, and a user-specified
	/// [`CommitmentLevel`] via [`CommitmentConfig`].
	pub fn new_with_commitment(endpoint: &str, commitment_config: CommitmentConfig) -> Self {
		println!("endpoint: {endpoint}");

		Self {
			provider: Arc::new(HttpProvider::new(endpoint)),
			commitment_config,
			ws: WebSocketProvider::new(endpoint),
		}
	}

	pub fn new_with_ws_and_commitment(
		http_endpoint: &str,
		ws_endpoint: &str,
		commitment_config: CommitmentConfig,
	) -> Self {
		Self {
			provider: Arc::new(HttpProvider::new(http_endpoint)),
			commitment_config,
			ws: WebSocketProvider::new(ws_endpoint),
		}
	}

	/// Create a new rpc from a custom provider.
	pub fn new_with_provider(
		provider: Arc<dyn RpcProvider + Send + Sync + 'static>,
		commitment_config: CommitmentConfig,
	) -> Self {
		let endpoint = provider.url();
		Self {
			provider,
			commitment_config,
			ws: WebSocketProvider::new(endpoint),
		}
	}

	/// Get the URL.
	pub fn url(&self) -> String {
		self.provider.url()
	}

	pub fn commitment(&self) -> CommitmentLevel {
		self.commitment_config.commitment
	}

	pub fn commitment_config(&self) -> CommitmentConfig {
		self.commitment_config
	}

	async fn send<T: HttpMethod, R: DeserializeOwned>(&self, request: T) -> ClientResult<R> {
		let result = self
			.provider
			.send(
				T::NAME,
				serde_json::to_value(request)
					.map_err(|error| ClientError::Other(error.to_string()))?,
			)
			.await?;

		match serde_json::from_value::<R>(result.clone()) {
			Ok(response) => Ok(response),
			_ => {
				match serde_json::from_value::<RpcError>(result) {
					Ok(error) => Err(error.into()),
					Err(error) => Err(ClientError::Other(error.to_string())),
				}
			}
		}
	}

	pub async fn get_account_with_config(
		&self,
		pubkey: &Pubkey,
		config: RpcAccountInfoConfig,
	) -> ClientResult<Option<Account>> {
		let request = GetAccountInfoRequest::builder()
			.pubkey(*pubkey)
			.config(config)
			.build();
		let response: ClientResponse<GetAccountInfoResponse> = self.send(request).await?;

		match response.result.value {
			Some(ui_account) => Ok(ui_account.decode()),
			None => Ok(None),
		}
	}

	pub async fn get_account_with_commitment(
		&self,
		pubkey: &Pubkey,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Option<Account>> {
		self.get_account_with_config(
			pubkey,
			RpcAccountInfoConfig {
				commitment: Some(commitment_config),
				encoding: Some(UiAccountEncoding::Base64),
				..Default::default()
			},
		)
		.await
	}

	pub async fn get_account(&self, pubkey: &Pubkey) -> ClientResult<Account> {
		let result = self
			.get_account_with_commitment(pubkey, self.commitment_config())
			.await?
			.ok_or_else(|| RpcError::new(format!("Account {pubkey} not found.")))?;

		Ok(result)
	}

	pub async fn get_account_data(&self, pubkey: &Pubkey) -> ClientResult<Vec<u8>> {
		Ok(self.get_account(pubkey).await?.data)
	}

	pub async fn get_balance_with_commitment(
		&self,
		pubkey: &Pubkey,
		commitment_config: CommitmentConfig,
	) -> ClientResult<u64> {
		let request = GetBalanceRequest::new_with_config(*pubkey, commitment_config);
		let response: ClientResponse<GetBalanceResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_balance(&self, pubkey: &Pubkey) -> ClientResult<u64> {
		self.get_balance_with_commitment(pubkey, self.commitment_config())
			.await
	}

	pub async fn request_airdrop(&self, pubkey: &Pubkey, lamports: u64) -> ClientResult<Signature> {
		let request =
			RequestAirdropRequest::new_with_config(*pubkey, lamports, self.commitment_config);
		let response: ClientResponse<RequestAirdropResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_signature_statuses(
		&self,
		signatures: &[Signature],
	) -> ClientResult<Vec<Option<TransactionStatus>>> {
		let request = GetSignatureStatusesRequest::new(signatures.into());
		let response: ClientResponse<GetSignatureStatusesResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_transaction_with_config(
		&self,
		signature: &Signature,
		config: RpcTransactionConfig,
	) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta> {
		let request = GetTransactionRequest::new_with_config(*signature, config);
		let response: ClientResponse<GetTransactionResponse> = self.send(request).await?;

		match response.result.into() {
			Some(result) => Ok(result),
			None => Err(RpcError::new(format!("Signature {signature} not found.")).into()),
		}
	}

	pub async fn get_transaction(
		&self,
		signature: &Signature,
	) -> ClientResult<EncodedConfirmedTransactionWithStatusMeta> {
		let request = GetTransactionRequest::new(*signature);
		let response: ClientResponse<GetTransactionResponse> = self.send(request).await?;

		match response.result.into() {
			Some(result) => Ok(result),
			None => Err(RpcError::new(format!("Signature {signature} not found.")).into()),
		}
	}

	pub async fn get_latest_blockhash_with_config(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<(Hash, u64)> {
		let request = GetLatestBlockhashRequest::new_with_config(commitment_config);
		let response: ClientResponse<GetLatestBlockhashResponse> = self.send(request).await?;

		Ok((
			response.result.value.blockhash,
			response.result.value.last_valid_block_height,
		))
	}

	pub async fn get_latest_blockhash_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<(Hash, u64)> {
		self.get_latest_blockhash_with_config(commitment_config)
			.await
	}

	pub async fn get_latest_blockhash(&self) -> ClientResult<Hash> {
		let result = self
			.get_latest_blockhash_with_commitment(self.commitment_config())
			.await?;

		Ok(result.0)
	}

	pub async fn is_blockhash_valid(
		&self,
		blockhash: &Hash,
		commitment_config: CommitmentConfig,
	) -> ClientResult<bool> {
		let request = IsBlockhashValidRequest::new_with_config(
			*blockhash,
			RpcContextConfig {
				commitment: Some(commitment_config),
				min_context_slot: None,
			},
		);
		let response: ClientResponse<IsBlockhashValidResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_minimum_balance_for_rent_exemption(
		&self,
		data_len: usize,
	) -> ClientResult<u64> {
		let request = GetMinimumBalanceForRentExemptionRequest::new(data_len);
		let response: ClientResponse<GetMinimumBalanceForRentExemptionResponse> =
			self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_fee_for_message(&self, message: &Message) -> ClientResult<u64> {
		let request = GetFeeForMessageRequest::new(message.to_owned());
		let response: ClientResponse<GetFeeForMessageResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn send_transaction_with_config(
		&self,
		transaction: &VersionedTransaction,
		config: RpcSendTransactionConfig,
	) -> ClientResult<Signature> {
		let transaction = transaction.to_owned();
		let transaction_signature = transaction.signatures[0];
		let request = SendTransactionRequest::new_with_config(transaction, config);
		let response: ClientResponse<SendTransactionResponse> = self.send(request).await?;
		let signature: Signature = response.result.into();

		// A mismatching RPC response signature indicates an issue with the RPC node,
		// and should not be passed along to confirmation methods. The transaction may
		// or may not have been submitted to the cluster, so callers should verify the
		// success of the correct transaction signature independently.
		if signature == transaction_signature {
			Ok(signature)
		} else {
			Err(RpcError::new(format!(
				"RPC node returned mismatched signature {signature:?}, expected \
				 {transaction_signature:?}"
			))
			.into())
		}
	}

	pub async fn send_transaction(
		&self,
		transaction: &VersionedTransaction,
	) -> ClientResult<Signature> {
		self.send_transaction_with_config(
			transaction,
			RpcSendTransactionConfig {
				preflight_commitment: Some(self.commitment()),
				encoding: Some(UiTransactionEncoding::Base64),
				..Default::default()
			},
		)
		.await
	}

	pub async fn confirm_transaction_with_commitment(
		&self,
		signature: &Signature,
		commitment_config: CommitmentConfig,
	) -> ClientResult<bool> {
		let mut is_success = false;

		for _ in 0..MAX_RETRIES {
			let signature_statuses = self.get_signature_statuses(&[*signature]).await?;

			if let Some(signature_status) = signature_statuses[0].as_ref() {
				if signature_status.confirmation_status.is_some() {
					let current_commitment = signature_status.confirmation_status.as_ref().unwrap();

					let commitment_matches = match commitment_config.commitment {
						CommitmentLevel::Finalized => {
							matches!(current_commitment, TransactionConfirmationStatus::Finalized)
						}
						CommitmentLevel::Confirmed => {
							matches!(
								current_commitment,
								TransactionConfirmationStatus::Finalized
									| TransactionConfirmationStatus::Confirmed
							)
						}
						CommitmentLevel::Processed => true,
					};
					if commitment_matches {
						is_success = signature_status.err.is_none();
						break;
					}
				}
			}

			Delay::new(Duration::from_millis(SLEEP_MS)).await;
		}

		Ok(is_success)
	}

	pub async fn confirm_transaction(&self, signature: &Signature) -> ClientResult<bool> {
		self.confirm_transaction_with_commitment(signature, self.commitment_config())
			.await
	}

	pub async fn send_and_confirm_transaction_with_config(
		&self,
		transaction: &VersionedTransaction,
		commitment_config: CommitmentConfig,
		config: RpcSendTransactionConfig,
	) -> ClientResult<Signature> {
		let tx_hash = self
			.send_transaction_with_config(transaction, config)
			.await?;

		self.confirm_transaction_with_commitment(&tx_hash, commitment_config)
			.await?;

		Ok(tx_hash)
	}

	pub async fn send_and_confirm_transaction_with_commitment(
		&self,
		transaction: &VersionedTransaction,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Signature> {
		self.send_and_confirm_transaction_with_config(
			transaction,
			commitment_config,
			RpcSendTransactionConfig {
				preflight_commitment: Some(commitment_config.commitment),
				encoding: Some(UiTransactionEncoding::Base64),
				..Default::default()
			},
		)
		.await
	}

	pub async fn send_and_confirm_transaction(
		&self,
		transaction: &VersionedTransaction,
	) -> ClientResult<Signature> {
		self.send_and_confirm_transaction_with_commitment(transaction, self.commitment_config())
			.await
	}

	pub async fn get_program_accounts_with_config(
		&self,
		pubkey: &Pubkey,
		config: RpcProgramAccountsConfig,
	) -> ClientResult<Vec<(Pubkey, Account)>> {
		let commitment = config
			.account_config
			.commitment
			.unwrap_or_else(|| self.commitment_config());
		let account_config = RpcAccountInfoConfig {
			commitment: Some(commitment),
			..config.account_config
		};
		let config = RpcProgramAccountsConfig {
			account_config,
			..config
		};

		let request = GetProgramAccountsRequest::new_with_config(*pubkey, config);
		let response: ClientResponse<GetProgramAccountsResponse> = self.send(request).await?;

		// Parse keyed accounts
		let accounts = response
			.result
			.keyed_accounts()
			.ok_or_else(|| RpcError::new("Program account doesn't exist."))?;

		let mut pubkey_accounts: Vec<(Pubkey, Account)> = Vec::with_capacity(accounts.len());
		for RpcKeyedAccount { pubkey, account } in accounts {
			pubkey_accounts.push((
				*pubkey,
				account
					.decode()
					.ok_or_else(|| RpcError::new(format!("Unable to decode {pubkey}")))?,
			));
		}
		Ok(pubkey_accounts)
	}

	pub async fn get_program_accounts(
		&self,
		pubkey: &Pubkey,
	) -> ClientResult<Vec<(Pubkey, Account)>> {
		self.get_program_accounts_with_config(
			pubkey,
			RpcProgramAccountsConfig {
				account_config: RpcAccountInfoConfig {
					encoding: Some(UiAccountEncoding::Base64),
					..RpcAccountInfoConfig::default()
				},
				..RpcProgramAccountsConfig::default()
			},
		)
		.await
	}

	pub async fn get_slot_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Slot> {
		let request = GetSlotRequest::new_with_config(commitment_config);
		let response: ClientResponse<GetSlotResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_slot(&self) -> ClientResult<Slot> {
		self.get_slot_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_block_with_config(
		&self,
		slot: Slot,
		config: RpcBlockConfig,
	) -> ClientResult<UiConfirmedBlock> {
		let request = GetBlockRequest::new_with_config(slot, config);
		let response: ClientResponse<GetBlockResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_version(&self) -> ClientResult<RpcVersionInfo> {
		let response: ClientResponse<GetVersionResponse> = self.send(GetVersionRequest).await?;

		Ok(response.result.into())
	}

	pub async fn get_first_available_block(&self) -> ClientResult<Slot> {
		let request = GetFirstAvailableBlockRequest;
		let response: ClientResponse<GetFirstAvailableBlockResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
		let request = GetBlockTimeRequest::new(slot);
		let response: ClientResponse<GetBlockTimeResponse> = self.send(request).await?;

		let maybe_timestamp: Option<UnixTimestamp> = response.result.into();
		match maybe_timestamp {
			Some(timestamp) => Ok(timestamp),
			None => Err(RpcError::new(format!("Block Not Found: slot={slot}")).into()),
		}
	}

	pub async fn get_block_height_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<u64> {
		let request = GetBlockHeightRequest::new_with_config(commitment_config);
		let response: ClientResponse<GetBlockHeightResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_block_height(&self) -> ClientResult<u64> {
		self.get_block_height_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_genesis_hash(&self) -> ClientResult<Hash> {
		let request = GetGenesisHashRequest;
		let response: ClientResponse<GetGenesisHashResponse> = self.send(request).await?;

		let hash_string: String = response.result.into();
		let hash = hash_string
			.parse()
			.map_err(|_| RpcError::new("Hash is not parseable."))?;

		Ok(hash)
	}

	pub async fn get_epoch_info_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<EpochInfo> {
		let request = GetEpochInfoRequest::new_with_config(commitment_config);
		let response: ClientResponse<GetEpochInfoResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_epoch_info(&self) -> ClientResult<EpochInfo> {
		self.get_epoch_info_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_recent_performance_samples_with_limit(
		&self,
		limit: usize,
	) -> ClientResult<Vec<RpcPerfSample>> {
		let request = GetRecentPerformanceSamplesRequest::new_with_limit(limit);
		let response: ClientResponse<GetRecentPerformanceSamplesResponse> =
			self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_recent_performance_samples(&self) -> ClientResult<Vec<RpcPerfSample>> {
		let request = GetRecentPerformanceSamplesRequest::new();
		let response: ClientResponse<GetRecentPerformanceSamplesResponse> =
			self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_recent_prioritization_fees(&self) -> ClientResult<Vec<RpcPrioritizationFee>> {
		let request = GetRecentPrioritizationFeesRequest::new();
		let response: ClientResponse<GetRecentPrioritizationFeesResponse> =
			self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_recent_prioritization_fees_with_accounts(
		&self,
		addresses: Vec<Pubkey>,
	) -> ClientResult<Vec<RpcPrioritizationFee>> {
		let request = GetRecentPrioritizationFeesRequest::new_with_accounts(addresses);
		let response: ClientResponse<GetRecentPrioritizationFeesResponse> =
			self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_blocks_with_limit_and_commitment(
		&self,
		start_slot: Slot,
		limit: usize,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Vec<Slot>> {
		let request =
			GetBlocksWithLimitRequest::new_with_config(start_slot, limit, commitment_config);
		let response: ClientResponse<GetBlocksWithLimitResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_blocks_with_limit(
		&self,
		start_slot: Slot,
		limit: usize,
	) -> ClientResult<Vec<Slot>> {
		self.get_blocks_with_limit_and_commitment(start_slot, limit, self.commitment_config())
			.await
	}

	pub async fn get_largest_accounts_with_config(
		&self,
		config: RpcLargestAccountsConfig,
	) -> ClientResult<Vec<RpcAccountBalance>> {
		let config = RpcLargestAccountsConfig {
			commitment: config.commitment,
			..config
		};

		let request = GetLargestAccountsRequest::new_with_config(config);
		let response: ClientResponse<GetLargestAccountsResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_supply_with_config(&self, config: RpcSupplyConfig) -> ClientResult<RpcSupply> {
		let request = GetSupplyRequest::new_with_config(config);
		let response: ClientResponse<GetSupplyResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_stake_minimum_delegation_with_commitment(
		&self,
		commitment: CommitmentLevel,
	) -> ClientResult<u64> {
		let request =
			GetStakeMinimumDelegationRequest::new_with_config(CommitmentConfig { commitment });
		let response: ClientResponse<GetStakeMinimumDelegationResponse> =
			self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_stake_minimum_delegation(&self) -> ClientResult<u64> {
		self.get_stake_minimum_delegation_with_commitment(self.commitment())
			.await
	}

	pub async fn get_supply_with_commitment(
		&self,
		commitment: CommitmentLevel,
	) -> ClientResult<RpcSupply> {
		self.get_supply_with_config(RpcSupplyConfig {
			commitment: Some(CommitmentConfig { commitment }),
			exclude_non_circulating_accounts_list: false,
		})
		.await
	}

	pub async fn get_supply(&self) -> ClientResult<RpcSupply> {
		self.get_supply_with_commitment(self.commitment()).await
	}

	pub async fn get_transaction_count_with_config(
		&self,
		config: RpcContextConfig,
	) -> ClientResult<u64> {
		let request = GetTransactionCountRequest::new_with_config(config);
		let response: ClientResponse<GetTransactionCountResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_transaction_count_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<u64> {
		self.get_transaction_count_with_config(RpcContextConfig {
			commitment: Some(commitment_config),
			min_context_slot: None,
		})
		.await
	}

	pub async fn get_transaction_count(&self) -> ClientResult<u64> {
		self.get_transaction_count_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_multiple_accounts_with_config(
		&self,
		pubkeys: &[Pubkey],
		config: RpcAccountInfoConfig,
	) -> ClientResult<Vec<Option<Account>>> {
		let config = RpcAccountInfoConfig {
			commitment: config.commitment,
			..config
		};

		let request = GetMultipleAccountsRequest::new_with_config(pubkeys.to_vec(), config);
		let response: ClientResponse<GetMultipleAccountsResponse> = self.send(request).await?;

		Ok(response
			.result
			.value
			.iter()
			.filter(|maybe_acc| maybe_acc.is_some())
			.map(|acc| acc.clone().unwrap().decode())
			.collect())
	}

	pub async fn get_multiple_accounts_with_commitment(
		&self,
		pubkeys: &[Pubkey],
		commitment_config: CommitmentConfig,
	) -> ClientResult<Vec<Option<Account>>> {
		self.get_multiple_accounts_with_config(
			pubkeys,
			RpcAccountInfoConfig {
				commitment: Some(commitment_config),
				..RpcAccountInfoConfig::default()
			},
		)
		.await
	}

	pub async fn get_multiple_accounts(
		&self,
		pubkeys: &[Pubkey],
	) -> ClientResult<Vec<Option<Account>>> {
		self.get_multiple_accounts_with_commitment(pubkeys, self.commitment_config())
			.await
	}

	pub async fn get_cluster_nodes(&self) -> ClientResult<Vec<RpcContactInfoWasm>> {
		let response: ClientResponse<GetClusterNodesResponse> =
			self.send(GetClusterNodesRequest).await?;

		Ok(response.result.into())
	}

	pub async fn get_vote_accounts_with_config(
		&self,
		config: RpcGetVoteAccountsConfig,
	) -> ClientResult<RpcVoteAccountStatus> {
		let request = GetVoteAccountsRequest::new_with_config(config);
		let response: ClientResponse<GetVoteAccountsResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_vote_accounts_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<RpcVoteAccountStatus> {
		self.get_vote_accounts_with_config(RpcGetVoteAccountsConfig {
			commitment: Some(commitment_config),
			..Default::default()
		})
		.await
	}

	pub async fn get_vote_accounts(&self) -> ClientResult<RpcVoteAccountStatus> {
		self.get_vote_accounts_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_epoch_schedule(&self) -> ClientResult<EpochSchedule> {
		let response: ClientResponse<GetEpochScheduleResponse> =
			self.send(GetEpochScheduleRequest).await?;

		Ok(response.result.into())
	}

	pub async fn get_signatures_for_address_with_config(
		&self,
		address: &Pubkey,
		config: GetConfirmedSignaturesForAddress2Config,
	) -> ClientResult<Vec<RpcConfirmedTransactionStatusWithSignature>> {
		let config = RpcSignaturesForAddressConfig {
			before: config.before,
			until: config.until,
			limit: config.limit,
			commitment: config.commitment,
			min_context_slot: None,
		};

		let request = GetSignaturesForAddressRequest::new_with_config(*address, config);
		let response: ClientResponse<GetSignaturesForAddressResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn minimum_ledger_slot(&self) -> ClientResult<Slot> {
		let response: ClientResponse<MinimumLedgerSlotResponse> =
			self.send(MinimumLedgerSlotRequest).await?;

		Ok(response.result.into())
	}

	pub async fn get_blocks_with_commitment(
		&self,
		start_slot: Slot,
		end_slot: Option<Slot>,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Vec<Slot>> {
		let request = GetBlocksRequest::new_with_config(start_slot, end_slot, commitment_config);
		let response: ClientResponse<GetBlocksResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_blocks(
		&self,
		start_slot: Slot,
		end_slot: Option<Slot>,
	) -> ClientResult<Vec<Slot>> {
		self.get_blocks_with_commitment(start_slot, end_slot, self.commitment_config())
			.await
	}

	pub async fn get_leader_schedule_with_config(
		&self,
		slot: Option<Slot>,
		config: RpcLeaderScheduleConfig,
	) -> ClientResult<Option<RpcLeaderSchedule>> {
		let request = match slot {
			Some(s) => GetLeaderScheduleRequest::new_with_slot_and_config(s, config),
			None => GetLeaderScheduleRequest::new_with_config(config),
		};
		let response: ClientResponse<GetLeaderScheduleResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_leader_schedule_with_commitment(
		&self,
		slot: Option<Slot>,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Option<RpcLeaderSchedule>> {
		self.get_leader_schedule_with_config(
			slot,
			RpcLeaderScheduleConfig {
				commitment: Some(commitment_config),
				..Default::default()
			},
		)
		.await
	}

	pub async fn get_block_production_with_config(
		&self,
		config: RpcBlockProductionConfig,
	) -> ClientResult<RpcBlockProduction> {
		let request = GetBlockProductionRequest::new_with_config(config);
		let response: ClientResponse<GetBlockProductionResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_block_production_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<RpcBlockProduction> {
		self.get_block_production_with_config(RpcBlockProductionConfig {
			commitment: Some(commitment_config),
			..Default::default()
		})
		.await
	}

	pub async fn get_block_production(&self) -> ClientResult<RpcBlockProduction> {
		self.get_block_production_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_inflation_governor_with_commitment(
		&self,
		commitment_config: CommitmentConfig,
	) -> ClientResult<RpcInflationGovernor> {
		let request = GetInflationGovernorRequest::new_with_config(commitment_config);
		let response: ClientResponse<GetInflationGovernorResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_inflation_governor(&self) -> ClientResult<RpcInflationGovernor> {
		self.get_inflation_governor_with_commitment(self.commitment_config())
			.await
	}

	pub async fn get_inflation_rate(&self) -> ClientResult<RpcInflationRate> {
		let response: ClientResponse<GetInflationRateResponse> =
			self.send(GetInflationRateRequest).await?;

		Ok(response.result.into())
	}

	pub async fn get_inflation_reward_with_config(
		&self,
		addresses: &[Pubkey],
		epoch: Option<Epoch>,
	) -> ClientResult<Vec<Option<RpcInflationReward>>> {
		let request = GetInflationRewardRequest::new_with_config(
			addresses.to_vec(),
			RpcEpochConfig {
				commitment: Some(self.commitment_config()),
				epoch,
				..Default::default()
			},
		);
		let response: ClientResponse<GetInflationRewardResponse> = self.send(request).await?;

		Ok(response.result.into())
	}

	pub async fn get_inflation_reward(
		&self,
		addresses: &[Pubkey],
	) -> ClientResult<Vec<Option<RpcInflationReward>>> {
		self.get_inflation_reward_with_config(addresses, None).await
	}

	pub async fn get_token_account_with_commitment(
		&self,
		pubkey: &Pubkey,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Option<UiTokenAccount>> {
		let config = RpcAccountInfoConfig {
			encoding: Some(UiAccountEncoding::JsonParsed),
			commitment: Some(commitment_config),
			data_slice: None,
			min_context_slot: None,
		};

		let request = GetAccountInfoRequest::builder()
			.pubkey(*pubkey)
			.config(config)
			.build();
		let response: ClientResponse<GetAccountInfoResponse> = self.send(request).await?;

		if let Some(acc) = response.result.value {
			if let UiAccountData::Json(account_data) = acc.data {
				let token_account_type: TokenAccountType =
					match serde_json::from_value(account_data.parsed) {
						Ok(t) => t,
						Err(e) => return Err(RpcError::new(e.to_string()).into()),
					};

				if let TokenAccountType::Account(token_account) = token_account_type {
					return Ok(Some(token_account));
				}
			}
		}

		Err(RpcError::new(format!("AccountNotFound: pubkey={pubkey}")).into())
	}

	pub async fn get_token_account(&self, pubkey: &Pubkey) -> ClientResult<Option<UiTokenAccount>> {
		self.get_token_account_with_commitment(pubkey, self.commitment_config())
			.await
	}

	pub async fn get_token_accounts_by_owner_with_commitment(
		&self,
		owner: &Pubkey,
		token_account_filter: TokenAccountsFilter,
		commitment_config: CommitmentConfig,
	) -> ClientResult<Vec<RpcKeyedAccount>> {
		let token_account_filter = match token_account_filter {
			TokenAccountsFilter::Mint(mint) => RpcTokenAccountsFilter::Mint(mint),
			TokenAccountsFilter::ProgramId(program_id) => {
				RpcTokenAccountsFilter::ProgramId(program_id)
			}
		};

		let config = RpcAccountInfoConfig {
			encoding: Some(UiAccountEncoding::JsonParsed),
			commitment: Some(commitment_config),
			data_slice: None,
			min_context_slot: None,
		};

		let request =
			GetTokenAccountsByOwnerRequest::new_with_config(*owner, token_account_filter, config);
		let response: ClientResponse<GetTokenAccountsByOwnerResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_token_accounts_by_owner(
		&self,
		owner: &Pubkey,
		token_account_filter: TokenAccountsFilter,
	) -> ClientResult<Vec<RpcKeyedAccount>> {
		self.get_token_accounts_by_owner_with_commitment(
			owner,
			token_account_filter,
			self.commitment_config(),
		)
		.await
	}

	pub async fn get_token_account_balance_with_commitment(
		&self,
		pubkey: &Pubkey,
		commitment_config: CommitmentConfig,
	) -> ClientResult<UiTokenAmount> {
		let request = GetTokenAccountBalanceRequest::new_with_config(*pubkey, commitment_config);
		let response: ClientResponse<GetTokenAccountBalanceResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_token_account_balance(&self, pubkey: &Pubkey) -> ClientResult<UiTokenAmount> {
		self.get_token_account_balance_with_commitment(pubkey, self.commitment_config())
			.await
	}

	pub async fn get_token_supply_with_commitment(
		&self,
		mint: &Pubkey,
		commitment_config: CommitmentConfig,
	) -> ClientResult<UiTokenAmount> {
		let request = GetTokenSupplyRequest::new_with_config(*mint, commitment_config);
		let response: ClientResponse<GetTokenSupplyResponse> = self.send(request).await?;

		Ok(response.result.value)
	}

	pub async fn get_token_supply(&self, mint: &Pubkey) -> ClientResult<UiTokenAmount> {
		self.get_token_supply_with_commitment(mint, self.commitment_config())
			.await
	}

	pub async fn simulate_transaction_with_config(
		&self,
		transaction: &VersionedTransaction,
		config: RpcSimulateTransactionConfig,
	) -> ClientResult<SimulateTransactionResponse> {
		let request = SimulateTransactionRequest::new_with_config(transaction.to_owned(), config);
		let response: ClientResponse<SimulateTransactionResponse> = self.send(request).await?;

		Ok(response.result)
	}

	/// Simulate the transaction without needing a signature.
	pub async fn simulate_transaction(
		&self,
		transaction: &VersionedTransaction,
	) -> ClientResult<SimulateTransactionResponse> {
		self.simulate_transaction_with_config(
			transaction,
			RpcSimulateTransactionConfig {
				encoding: Some(UiTransactionEncoding::Base64),
				replace_recent_blockhash: Some(true),
				..Default::default()
			},
		)
		.await
	}

	pub async fn get_health(&self) -> ClientResult<GetHealthResponse> {
		let response: ClientResponse<GetHealthResponse> = self.send(GetHealthRequest).await?;

		Ok(response.result)
	}

	/// Returns the identity pubkey for the current node.
	///
	/// # RPC Reference
	///
	/// This method corresponds directly to the [`getIdentity`] RPC method.
	///
	/// [`getIdentity`]: https://solana.com/docs/rpc/http/getidentity
	pub async fn get_identity(&self) -> ClientResult<GetIdentityResponse> {
		let response: ClientResponse<GetIdentityResponse> = self.send(GetIdentityRequest).await?;

		Ok(response.result)
	}

	/// Returns commitment for particular block
	pub async fn get_block_commitment(
		&self,
		slot: u64,
	) -> ClientResult<GetBlockCommitmentResponse> {
		let request = GetBlockCommitmentRequest::new(slot);
		let response: ClientResponse<GetBlockCommitmentResponse> = self.send(request).await?;

		Ok(response.result)
	}

	/// Returns the highest slot information that the node has snapshots for.
	/// This will find the highest full snapshot slot, and the highest
	/// incremental snapshot slot based on the full snapshot slot, if there is
	/// one.
	///
	/// *VERSION RESTRICTION*
	/// This method is only available in solana-core v1.9 or newer. Please use
	/// getSnapshotSlot for solana-core v1.8 and below.
	pub async fn get_highest_snapshot_slot(&self) -> ClientResult<GetHighestSnapshotSlotResponse> {
		let response: ClientResponse<GetHighestSnapshotSlotResponse> =
			self.send(GetHighestSnapshotSlotRequest).await?;

		Ok(response.result)
	}

	/// Get the max slot seen from retransmit stage.
	pub async fn get_max_retransmit_slot(&self) -> ClientResult<GetMaxRetransmitSlotResponse> {
		let response: ClientResponse<GetMaxRetransmitSlotResponse> =
			self.send(GetMaxRetransmitSlotRequest).await?;

		Ok(response.result)
	}

	/// Returns the current slot leader
	pub async fn get_slot_leader(&self) -> ClientResult<GetSlotLeaderResponse> {
		let request = GetSlotLeaderRequest::new();
		let response: ClientResponse<GetSlotLeaderResponse> = self.send(request).await?;

		Ok(response.result)
	}

	/// Returns the slot leaders for a given slot range
	pub async fn get_slot_leaders_with_config(
		&self,
		start_slot: u64,
		limit: u64,
	) -> ClientResult<GetSlotLeadersResponse> {
		let request = GetSlotLeadersRequest::new_with_config(start_slot, limit);
		let response: ClientResponse<GetSlotLeadersResponse> = self.send(request).await?;

		Ok(response.result)
	}

	/// Returns the slot leaders for a given slot range
	pub async fn get_slot_leaders(&self) -> ClientResult<GetSlotLeadersResponse> {
		let request = GetSlotLeadersRequest::new();
		let response: ClientResponse<GetSlotLeadersResponse> = self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_stake_activation(
		&self,
		pubkey: Pubkey,
	) -> ClientResult<GetStakeActivationResponse> {
		let request = GetStakeActivationRequest::new(pubkey);
		let response: ClientResponse<GetStakeActivationResponse> = self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_stake_activation_with_config(
		&self,
		pubkey: Pubkey,
		config: RpcEpochConfig,
	) -> ClientResult<GetStakeActivationResponse> {
		let request = GetStakeActivationRequest::new_with_config(pubkey, config);
		let response: ClientResponse<GetStakeActivationResponse> = self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_token_accounts_by_delegate_with_config(
		&self,
		pubkey: Pubkey,
		filter: RpcTokenAccountsFilter,
		config: RpcAccountInfoConfig,
	) -> ClientResult<GetTokenAccountsByDelegateResponse> {
		let request = GetTokenAccountsByDelegateRequest {
			pubkey,
			filter,
			config: Some(config),
		};
		let response: ClientResponse<GetTokenAccountsByDelegateResponse> =
			self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_token_accounts_by_delegate(
		&self,
		pubkey: Pubkey,
		filter: RpcTokenAccountsFilter,
	) -> ClientResult<GetTokenAccountsByDelegateResponse> {
		let request = GetTokenAccountsByDelegateRequest {
			pubkey,
			filter,
			config: None,
		};
		let response: ClientResponse<GetTokenAccountsByDelegateResponse> =
			self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_token_largest_accounts(
		&self,
		pubkey: Pubkey,
	) -> ClientResult<GetTokenLargestAccountsResponse> {
		let request = GetTokenLargestAccountsRequest::new(pubkey);
		let response: ClientResponse<GetTokenLargestAccountsResponse> = self.send(request).await?;

		Ok(response.result)
	}

	pub async fn get_token_largest_accounts_with_config(
		&self,
		pubkey: Pubkey,
		config: CommitmentConfig,
	) -> ClientResult<GetTokenLargestAccountsResponse> {
		let request = GetTokenLargestAccountsRequest::new_with_config(pubkey, config);
		let response: ClientResponse<GetTokenLargestAccountsResponse> = self.send(request).await?;

		Ok(response.result)
	}

	/// Get the address lookup table.
	pub async fn get_address_lookup_table(
		&self,
		pubkey: &Pubkey,
	) -> ClientResult<LookupTableAccountType> {
		let account = self.get_account(pubkey).await?;
		let table_type = parse_address_lookup_table(&account.data)
			.map_err(|error| RpcError::new(error.to_string()))?;

		Ok(table_type)
	}

	/// Wait for the new block which is `n` blocks in the future.
	pub async fn wait_for_new_block(&self, n: u8) -> ClientResult<()> {
		let (_, last_valid_block_height) = self
			.get_latest_blockhash_with_commitment(self.commitment_config())
			.await?;

		for _ in 0..MAX_RETRIES {
			let (_, latest) = self
				.get_latest_blockhash_with_commitment(self.commitment_config())
				.await?;

			if latest >= last_valid_block_height + u64::from(n) {
				break;
			}

			Delay::new(Duration::from_millis(SLEEP_MS)).await;
		}

		Ok(())
	}

	/// Subscribe to account events with config.
	///
	/// Receives messages of type [`GetAccountInfoResponse`] when an account's
	/// lamports or data changes.
	///
	/// # RPC Reference
	///
	/// This method corresponds directly to the [`accountSubscribe`] RPC method.
	///
	/// [`accountSubscribe`]: https://docs.solana.com/api/websocket#accountsubscribe
	///
	/// ```rust
	/// # use wasm_client_solana::DEVNET;
	/// # use wasm_client_solana::SolanaRpcClient;
	/// # use solana_sdk::pubkey;
	/// # use futures::StreamExt;
	///
	/// # async fn run() -> anyhow::Result<()> {
	/// let pubkey = pubkey!("99P8ZgtJYe1buSK8JXkvpLh8xPsCFuLYhz9hQFNw93WJ");
	/// let mut client = SolanaRpcClient::new(DEVNET);
	/// let mut subscription = client.account_subscribe(&pubkey).await?;
	///
	/// while let Some(notification) = subscription.next().await {
	/// 	println!("Notification: {notification:?}");
	/// }
	///
	/// # Ok(())
	/// # }
	/// ```
	pub async fn account_subscribe(
		&self,
		request: impl Into<GetAccountInfoRequest>,
	) -> ClientResult<Subscription<GetAccountInfoResponse>> {
		let request: GetAccountInfoRequest = request.into();
		let (id, subscription_id) = self.ws.create_subscription(request).await?;
		let subscription = Subscription::new(&self.ws, id, subscription_id);

		Ok(subscription)
	}

	/// Subscribe to block events.
	///
	/// Receives messages of type [`RpcBlockUpdate`] when a block is confirmed
	/// or finalized.
	///
	/// This method is disabled by default. It can be enabled by passing
	/// `--rpc-pubsub-enable-block-subscription` to `solana-validator`.
	///
	/// # RPC Reference
	///
	/// This method corresponds directly to the [`blockSubscribe`] RPC method.
	///
	/// [`blockSubscribe`]: https://docs.solana.com/api/websocket#blocksubscribe
	pub async fn block_subscribe(
		&self,
		request: BlockSubscribeRequest,
	) -> ClientResult<Subscription<BlockNotificationResponse>> {
		let (id, subscription_id) = self.ws.create_subscription(request).await?;
		let subscription = Subscription::new(&self.ws, id, subscription_id);

		Ok(subscription)
	}

	/// Subscribe to transaction log events.
	///
	/// Receives messages of type [`RpcLogsResponse`] when a transaction is
	/// committed.
	///
	/// # RPC Reference
	///
	/// This method corresponds directly to the [`logsSubscribe`] RPC method.
	///
	/// [`logsSubscribe`]: https://docs.solana.com/api/websocket#logssubscribe
	pub async fn logs_subscribe(
		&self,
		request: LogsSubscribeRequest,
	) -> ClientResult<Subscription<LogsNotificationResponse>> {
		let (id, subscription_id) = self.ws.create_subscription(request).await?;
		let subscription = Subscription::new(&self.ws, id, subscription_id);

		Ok(subscription)
	}

	/// Subscribe to program account events.
	///
	/// Receives messages of type [`GetProgramAccountsResponse`] when an account
	/// owned by the given program changes.
	///
	/// # RPC Reference
	///
	/// This method corresponds directly to the [`programSubscribe`] RPC method.
	///
	/// [`programSubscribe`]: https://docs.solana.com/api/websocket#programsubscribe
	pub async fn program_subscribe(
		&self,
		request: ProgramSubscribeRequest,
	) -> ClientResult<Subscription<GetProgramAccountsResponse>> {
		let (id, subscription_id) = self.ws.create_subscription(request).await?;
		let subscription = Subscription::new(&self.ws, id, subscription_id);

		Ok(subscription)
	}
}
