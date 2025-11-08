//! Durable transaction nonce helpers.

use serde::Deserialize;
use serde::Serialize;
use solana_account::Account;
use solana_account::ReadableAccount;
use solana_account::state_traits::StateMut;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_nonce::state::Data;
use solana_nonce::state::State;
use solana_nonce::versions::Versions;
use solana_pubkey::Pubkey;
use solana_sdk_ids::system_program;

use crate::ClientResult;
use crate::SolanaRpcClient;
use crate::rpc_config::RpcAccountInfoConfig;

#[derive(Clone, Debug, Serialize, Deserialize, thiserror::Error, PartialEq, Eq)]
pub enum NonceError {
	#[error("invalid account owner")]
	InvalidAccountOwner,
	#[error("invalid account data")]
	InvalidAccountData,
	#[error("unexpected account data size")]
	UnexpectedDataSize,
	#[error("provided hash ({provided}) does not match nonce hash ({expected})")]
	InvalidHash { provided: Hash, expected: Hash },
	#[error("provided authority ({provided}) does not match nonce authority ({expected})")]
	InvalidAuthority { provided: Pubkey, expected: Pubkey },
	#[error("invalid state for requested operation")]
	InvalidStateForOperation,
	#[error("client error: {0}")]
	Client(String),
}

/// Get a nonce account from the network.
///
/// This is like [`SolanaRpcClient::get_account`] except:
///
/// - it returns this module's [`NonceError`] type,
/// - it returns an error if any of the checks from [`account_identity_ok`]
///   fail.
pub async fn get_account(
	rpc_client: &SolanaRpcClient,
	nonce_pubkey: &Pubkey,
) -> Result<Account, NonceError> {
	get_account_with_commitment(rpc_client, nonce_pubkey, CommitmentConfig::default()).await
}

/// Get a nonce account from the network.
///
/// This is like [`SolanaRpcClient::get_account_with_commitment`] except:
///
/// - it returns this module's [`NonceError`] type,
/// - it returns an error if the account does not exist,
/// - it returns an error if any of the checks from [`account_identity_ok`]
///   fail.
pub async fn get_account_with_commitment(
	rpc_client: &SolanaRpcClient,
	nonce_pubkey: &Pubkey,
	commitment_config: CommitmentConfig,
) -> Result<Account, NonceError> {
	rpc_client
		.get_account_with_config(
			nonce_pubkey,
			RpcAccountInfoConfig {
				commitment: Some(commitment_config),
				..Default::default()
			},
		)
		.await
		.map_err(|e| NonceError::Client(format!("{e}")))
		.and_then(|opt| {
			opt.ok_or_else(|| NonceError::Client(format!("AccountNotFound: pubkey={nonce_pubkey}")))
		})
		.and_then(|a| account_identity_ok(&a).map(|()| a))
}

/// Perform basic checks that an account has nonce-like properties.
///
/// # Errors
///
/// Returns [`NonceError::InvalidAccountOwner`] if the account is not owned by
/// the system program. Returns [`NonceError::UnexpectedDataSize`] if the
/// account contains no data.
pub fn account_identity_ok<T: ReadableAccount>(account: &T) -> Result<(), NonceError> {
	if account.owner() != &system_program::ID {
		Err(NonceError::InvalidAccountOwner)
	} else if account.data().is_empty() {
		Err(NonceError::UnexpectedDataSize)
	} else {
		Ok(())
	}
}

/// Deserialize the state of a durable transaction nonce account.
///
/// # Errors
///
/// Returns an error if the account is not owned by the system program or
/// contains no data.
pub fn state_from_account<T: ReadableAccount + StateMut<Versions>>(
	account: &T,
) -> Result<State, NonceError> {
	account_identity_ok(account)?;
	StateMut::<Versions>::state(account)
		.map_err(|_| NonceError::InvalidAccountData)
		.map(|v| v.state().clone())
}

/// Deserialize the state data of a durable transaction nonce account.
///
/// # Errors
///
/// Returns an error if the account is not owned by the system program or
/// contains no data. Returns an error if the account state is uninitialized or
/// fails to deserialize.
pub fn data_from_account<T: ReadableAccount + StateMut<Versions>>(
	account: &T,
) -> Result<Data, NonceError> {
	account_identity_ok(account)?;
	state_from_account(account).and_then(|ref s| data_from_state(s).cloned())
}

/// Get the nonce data from its [`State`] value.
///
/// # Errors
///
/// Returns [`NonceError::InvalidStateForOperation`] if `state` is
/// [`State::Uninitialized`].
pub fn data_from_state(state: &State) -> Result<&Data, NonceError> {
	match state {
		State::Uninitialized => Err(NonceError::InvalidStateForOperation),
		State::Initialized(data) => Ok(data),
	}
}

impl SolanaRpcClient {
	/// Deserialize the state of a durable transaction nonce account.
	///
	/// # Errors
	///
	/// Returns an error if the account is not owned by the system program or
	/// contains no data.
	pub async fn get_nonce_account_state(&self, nonce_pubkey: &Pubkey) -> ClientResult<State> {
		let account = self.get_account(nonce_pubkey).await?;
		account_identity_ok(&account)?;
		let state = state_from_account(&account)?;

		Ok(state)
	}

	/// Deserialize the state data of a durable transaction nonce account.
	///
	/// # Errors
	///
	/// Returns an error if the account is not owned by the system program or
	/// contains no data. Returns an error if the account state is uninitialized
	/// or fails to deserialize.
	pub async fn get_nonce_account_data(&self, nonce_pubkey: &Pubkey) -> ClientResult<Data> {
		let state = self.get_nonce_account_state(nonce_pubkey).await?;
		let data = data_from_state(&state).cloned()?;

		Ok(data)
	}
}
