//! Request configuration types for the JSON-RPC methods.
//!
//! Every `Rpc*Config` struct maps onto the `config` object of the corresponding
//! RPC method, and serializes with `camelCase` field names. Fields left as
//! `None` are omitted from the request rather than sent as `null`, so the node
//! applies its own default.
//!
//! Read configs default `max_supported_transaction_version` to `1` using
//! [`MAX_SUPPORTED_TRANSACTION_VERSION`] because the `txv1` feature gate is
//! active on mainnet.

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_clock::Epoch;
use solana_clock::Slot;
use solana_commitment_config::CommitmentConfig;
use solana_commitment_config::CommitmentLevel;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use typed_builder::TypedBuilder;
use wincode::SchemaRead;
use wincode::SchemaWrite;
use wincode::config::DefaultConfig;

use super::rpc_filter::RpcFilterType;
use crate::ClientError;
use crate::ClientResult;
use crate::MAX_SUPPORTED_TRANSACTION_VERSION;
use crate::RpcError;
use crate::SolanaRpcClient;
use crate::impl_websocket_method;
use crate::nonce_utils;
use crate::solana_account_decoder::UiAccount;
use crate::solana_account_decoder::UiAccountEncoding;
use crate::solana_account_decoder::UiDataSliceConfig;
use crate::solana_transaction_status::TransactionDetails;
use crate::solana_transaction_status::UiTransactionEncoding;

/// A public key paired with the account it identifies.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcKeyedAccount {
	/// Address of the account, as a base-58 encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	/// The account at [`RpcKeyedAccount::pubkey`].
	pub account: UiAccount,
}

/// Where a recent blockhash is fetched from.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
	/// The cluster's most recent blockhash.
	Cluster,
	/// The blockhash stored in the given durable nonce account.
	NonceAccount(Pubkey),
}

impl Source {
	/// Fetch a recent blockhash from this source.
	///
	/// # Errors
	///
	/// Returns an error if the RPC request fails or the nonce account cannot be
	/// read.
	pub async fn get_blockhash(
		&self,
		rpc_client: &SolanaRpcClient,
		commitment_config: CommitmentConfig,
	) -> Result<Hash, Box<dyn std::error::Error>> {
		match self {
			Self::Cluster => {
				let (blockhash, _) = rpc_client
					.get_latest_blockhash_with_config(commitment_config)
					.await?;
				Ok(blockhash)
			}
			Self::NonceAccount(pubkey) => {
				#[allow(clippy::redundant_closure)]
				let data = nonce_utils::get_account_with_commitment(rpc_client, pubkey, commitment_config)
					.await
					.and_then(|ref a| nonce_utils::data_from_account(a))?;
				Ok(data.blockhash())
			}
		}
	}

	/// Check whether a blockhash is still valid for this source.
	///
	/// For a cluster blockhash this asks the node whether the hash is still
	/// usable.
	///
	/// For a nonce account the account is only read, not compared against
	/// `blockhash`. A durable nonce does not expire with age, so any readable
	/// nonce account reports `true` — including one whose nonce has already
	/// been advanced, which makes a stale hash appear valid. This matches the
	/// behavior of the upstream `solana-rpc-client-nonce-utils` crate.
	///
	/// # Errors
	///
	/// Returns an error if the RPC request fails or the nonce account cannot be
	/// read.
	pub async fn is_blockhash_valid(
		&self,
		rpc_client: &SolanaRpcClient,
		blockhash: &Hash,
		commitment_config: CommitmentConfig,
	) -> Result<bool, Box<dyn std::error::Error>> {
		Ok(match self {
			Self::Cluster => {
				rpc_client
					.is_blockhash_valid(blockhash, commitment_config)
					.await?
			}
			Self::NonceAccount(pubkey) => {
				#[allow(clippy::redundant_closure)]
				let _ = nonce_utils::get_account_with_commitment(rpc_client, pubkey, commitment_config)
					.await
					.and_then(|ref a| nonce_utils::data_from_account(a))?;
				true
			}
		})
	}
}

/// How a transaction should obtain its recent blockhash.
#[derive(Debug, PartialEq, Eq)]
pub enum BlockhashQuery {
	/// Use the given blockhash as is, without checking it against the cluster.
	None(Hash),
	/// Validate the given blockhash against the cluster and fail if it expired.
	FeeCalculator(Source, Hash),
	/// Fetch a fresh blockhash from the source.
	All(Source),
}

impl BlockhashQuery {
	/// Choose a query strategy from the caller's inputs.
	///
	/// When `sign_only` is set the transaction is being signed offline, so no
	/// cluster round trip is made and the blockhash is used as given.
	///
	/// # Panics
	///
	/// Panics when `sign_only` is set but no `blockhash` was supplied, because
	/// there is nothing to sign against.
	pub fn new(blockhash: Option<Hash>, sign_only: bool, nonce_account: Option<Pubkey>) -> Self {
		let source = nonce_account.map_or(Source::Cluster, Source::NonceAccount);
		match blockhash {
			Some(hash) if sign_only => Self::None(hash),
			Some(hash) if !sign_only => Self::FeeCalculator(source, hash),
			None if !sign_only => Self::All(source),
			_ => panic!("Cannot resolve blockhash"),
		}
	}

	/// Resolve this query into a usable recent blockhash.
	///
	/// # Errors
	///
	/// Returns an error if the blockhash has expired or the cluster cannot be
	/// reached.
	pub async fn get_blockhash(
		&self,
		rpc_client: &SolanaRpcClient,
		commitment_config: CommitmentConfig,
	) -> Result<Hash, Box<dyn std::error::Error>> {
		match self {
			BlockhashQuery::None(hash) => Ok(*hash),
			BlockhashQuery::FeeCalculator(source, hash) => {
				if !source
					.is_blockhash_valid(rpc_client, hash, commitment_config)
					.await?
				{
					return Err(format!("Hash has expired {hash:?}").into());
				}
				Ok(*hash)
			}
			BlockhashQuery::All(source) => {
				source.get_blockhash(rpc_client, commitment_config).await
			}
		}
	}
}

impl Default for BlockhashQuery {
	/// Default to fetching a fresh blockhash from the cluster.
	fn default() -> Self {
		BlockhashQuery::All(Source::Cluster)
	}
}

/// Serialize a transaction-like value using the wire format and encode it as a
/// string.
///
/// <!-- {=txv1WireFormat|trim|linePrefix:"/// ":true} -->
/// Transaction bytes use the version dependent `wincode` wire format. Legacy
/// and v0 transactions place a `short_vec` signature count first; v1
/// transactions place the message first behind a `0x81` discriminator and move
/// the signatures to the tail. <!-- {/txv1WireFormat} -->
///
/// `wincode` emits the correct layout for every version, so this must be used
/// instead of a serde based format such as `bincode`, which produces bytes the
/// cluster rejects for v1.
pub fn serialize_and_encode<T>(input: &T, encoding: UiTransactionEncoding) -> ClientResult<String>
where
	T: SchemaWrite<DefaultConfig, Src = T>,
{
	let serialized = wincode::serialize(input)
		.map_err(|e| RpcError::new(format!("Serialization failed: {e}")))?;
	let encoded = match encoding {
		UiTransactionEncoding::Base58 => bs58::encode(serialized).into_string(),
		UiTransactionEncoding::Base64 => BASE64_STANDARD.encode(serialized),
		_ => {
			return Err(RpcError::new(format!(
				"unsupported encoding: {encoding}. Supported encodings: base58, base64"
			))
			.into());
		}
	};
	Ok(encoded)
}

/// Decode a string produced by [`serialize_and_encode`] back into a value.
///
/// # Errors
///
/// Returns an error when the encoding is not base58 or base64, the payload is
/// not valid for that encoding, or the bytes do not match the wire format.
pub fn deserialize_and_decode<T>(content: &str, encoding: UiTransactionEncoding) -> ClientResult<T>
where
	T: for<'de> SchemaRead<'de, DefaultConfig, Dst = T>,
{
	let decoded = match encoding {
		UiTransactionEncoding::Base64 => {
			wincode::deserialize(
				&BASE64_STANDARD
					.decode(content)
					.map_err(|e| ClientError::Other(e.to_string()))?,
			)
			.map_err(|e| ClientError::Other(e.to_string()))?
		}
		UiTransactionEncoding::Base58 => {
			wincode::deserialize(
				&bs58::decode(content)
					.into_vec()
					.map_err(|e| ClientError::Other(e.to_string()))?,
			)
			.map_err(|e| ClientError::Other(e.to_string()))?
		}
		_ => {
			return Err(RpcError::new(format!(
				"unsupported encoding: {encoding}. Supported encodings: base58, base64"
			))
			.into());
		}
	};

	Ok(decoded)
}

/// Configuration for `getSignatureStatuses`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureStatusConfig {
	/// Search the ledger's transaction history rather than only the recent
	/// status cache. Slower, but can find statuses for older transactions.
	pub search_transaction_history: bool,
}

/// Configuration for `sendTransaction`.
#[skip_serializing_none]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSendTransactionConfig {
	/// Skip the preflight transaction checks.
	///
	/// Preflight simulation catches most errors before a transaction is
	/// broadcast, so disabling it trades a clearer error for latency. Leave it
	/// enabled unless you are retrying a transaction that already simulated
	/// successfully.
	#[serde(default)]
	#[builder(!default, setter(into, !strip_option, strip_bool(fallback = skip_preflight_bool)))]
	pub skip_preflight: bool,
	/// Commitment level to use for preflight.
	pub preflight_commitment: Option<CommitmentLevel>,
	/// <!-- {=encodingTransaction|trim|linePrefix:"/// ":true} -->
	/// Encoding format for transaction data. Use `base64` for anything that may
	/// exceed the 1232-byte base58 limit, such as a v1 transaction.
	/// <!-- {/encodingTransaction} -->
	pub encoding: Option<UiTransactionEncoding>,
	/// The maximum number of times for the RPC node to retry sending the
	/// transaction to the leader.
	pub max_retries: Option<usize>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	pub min_context_slot: Option<Slot>,
}

/// Account encoding and addresses for `simulateTransaction`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionAccountsConfig {
	/// <!-- {=encodingAccount|trim|linePrefix:"/// ":true} -->
	/// Encoding format for account data.
	/// <!-- {/encodingAccount} -->
	pub encoding: Option<UiAccountEncoding>,
	/// Addresses of the accounts to return, as base-58 encoded strings.
	#[builder(setter(!strip_option))]
	pub addresses: Vec<String>,
}

/// Configuration for `simulateTransaction`.
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionConfig {
	/// Verify the transaction's signatures while simulating.
	///
	/// Only needed when calling `simulateTransaction` directly, since
	/// `sendTransaction` requires signatures to be valid regardless.
	#[serde(default)]
	#[builder(setter(into, strip_bool(fallback = sig_verify_bool)))]
	pub sig_verify: bool,
	/// Replace the transaction's recent blockhash with the cluster's latest.
	///
	/// Useful when simulating a transaction that was signed earlier and whose
	/// blockhash may have expired.
	#[serde(default)]
	#[builder(default, setter(into, strip_option(fallback = replace_recent_blockhash_opt)))]
	pub replace_recent_blockhash: Option<bool>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	#[builder(default, setter(into, strip_option(fallback = commitment_opt)))]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=encodingTransaction|trim|linePrefix:"/// ":true} -->
	/// Encoding format for transaction data. Use `base64` for anything that may
	/// exceed the 1232-byte base58 limit, such as a v1 transaction.
	/// <!-- {/encodingTransaction} -->
	#[builder(default, setter(into, strip_option(fallback = encoding_opt)))]
	pub encoding: Option<UiTransactionEncoding>,
	/// Return the accounts the transaction would modify, along with their
	/// post-simulation data.
	#[builder(default, setter(into, strip_option(fallback = accounts_opt)))]
	pub accounts: Option<RpcSimulateTransactionAccountsConfig>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	#[builder(default, setter(into, strip_option(fallback = min_context_slot_opt)))]
	pub min_context_slot: Option<Slot>,
}

/// Configuration for `requestAirdrop`.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcRequestAirdropConfig {
	/// Blockhash to use for the airdrop transaction. The node uses its latest
	/// blockhash when omitted.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub recent_blockhash: Option<Hash>, // base-58 encoded blockhash
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

/// Configuration for `getLeaderSchedule`.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcLeaderScheduleConfig {
	/// Only return the schedule for this validator identity, as a base-58
	/// encoded string.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub identity: Option<Pubkey>, // validator identity, as a base-58 encoded string
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

/// A range of slots to report block production over.
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProductionConfigRange {
	/// First slot of the range, inclusive.
	pub first_slot: Slot,
	/// Last slot of the range, inclusive. The current epoch's last slot is used
	/// when omitted.
	#[builder(default, setter(into, strip_option(fallback = last_slot_opt)))]
	pub last_slot: Option<Slot>,
}

/// Configuration for `getBlockProduction`.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProductionConfig {
	/// Only return block production for this validator identity, as a base-58
	/// encoded string.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub identity: Option<Pubkey>, // validator identity, as a base-58 encoded string
	/// Slots to report over. Defaults to the current epoch.
	pub range: Option<RpcBlockProductionConfigRange>, // current epoch if `None`
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

/// Configuration for `getVoteAccounts`.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcGetVoteAccountsConfig {
	/// Only return the vote account with this address, as a base-58 encoded
	/// string.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub vote_pubkey: Option<Pubkey>, // validator vote address, as a base-58 encoded string
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// Include vote accounts that have no stake.
	pub keep_unstaked_delinquents: Option<bool>,
	/// How many slots a validator may fall behind before it is reported as
	/// delinquent.
	pub delinquent_slot_distance: Option<u64>,
}

/// Either a slot or a [`RpcLeaderScheduleConfig`], depending on which form the
/// caller used.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcLeaderScheduleConfigWrapper {
	/// Request the schedule for a single slot.
	SlotOnly(Option<Slot>),
	/// Request the schedule with a full config.
	ConfigOnly(Option<RpcLeaderScheduleConfig>),
}

impl RpcLeaderScheduleConfigWrapper {
	/// Split into the slot and config halves.
	pub fn unzip(&self) -> (Option<Slot>, Option<RpcLeaderScheduleConfig>) {
		match &self {
			RpcLeaderScheduleConfigWrapper::SlotOnly(slot) => (*slot, None),
			RpcLeaderScheduleConfigWrapper::ConfigOnly(config) => (None, config.clone()),
		}
	}
}

/// Which set of largest accounts to return.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcLargestAccountsFilter {
	/// Accounts whose lamports count towards the circulating supply.
	Circulating,
	/// Accounts excluded from the circulating supply, such as foundation and
	/// lockup accounts.
	NonCirculating,
}

/// Configuration for `getLargestAccounts`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcLargestAccountsConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// Which accounts to consider.
	pub filter: Option<RpcLargestAccountsFilter>,
}

/// Configuration for `getSupply`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSupplyConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// Omit the list of non-circulating accounts from the response.
	#[serde(default)]
	pub exclude_non_circulating_accounts_list: bool,
}

/// Configuration for `getEpochInfo` and `getInflationReward`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcEpochConfig {
	/// Epoch to query. Defaults to the current epoch.
	pub epoch: Option<Epoch>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	pub min_context_slot: Option<Slot>,
}

/// Configuration for methods that return a single account.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcAccountInfoConfig {
	/// <!-- {=encodingAccount|trim|linePrefix:"/// ":true} -->
	/// Encoding format for account data.
	/// <!-- {/encodingAccount} -->
	///
	/// Defaults to `base64`, which round trips any account regardless of size.
	#[builder(default = Some(UiAccountEncoding::Base64), setter(into, strip_option(fallback = encoding_opt)))]
	pub encoding: Option<UiAccountEncoding>,
	/// Return only a slice of the account data, given an offset and length.
	/// Useful for accounts too large to fetch in full.
	#[builder(default, setter(into, strip_option(fallback = data_slice_opt)))]
	pub data_slice: Option<UiDataSliceConfig>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	#[builder(default, setter(into, strip_option(fallback = commitment_opt)))]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	#[builder(default, setter(into, strip_option(fallback = min_context_slot_opt)))]
	pub min_context_slot: Option<Slot>,
}

/// Configuration for `getProgramAccounts`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcProgramAccountsConfig {
	/// Filter results using the provided filters.
	///
	/// Filters are applied by the node. Data size and memcmp filters are cheap;
	/// a memcmp filter without an offset forces the node to scan for the bytes
	/// anywhere in the account data.
	#[builder(default, setter(strip_option(fallback = filters_opt)))]
	pub filters: Option<Vec<RpcFilterType>>,
	/// Account encoding, data slice and commitment options.
	#[serde(flatten)]
	pub account_config: RpcAccountInfoConfig,
	/// Wrap the response in a `context` object carrying the slot the data was
	/// fetched at.
	#[builder(default, setter(strip_option(fallback = with_context_opt)))]
	pub with_context: Option<bool>,
}

/// A `programSubscribe` request.
#[derive(Debug, Clone, Default, PartialEq, Eq, TypedBuilder)]
pub struct ProgramSubscribeRequest {
	/// Program whose accounts should be tracked.
	pub program_id: Pubkey,
	/// Filters and encoding options for the accounts.
	#[builder(default, setter(strip_option(fallback = config_opt)))]
	pub config: Option<RpcProgramAccountsConfig>,
}

impl_websocket_method!(ProgramSubscribeRequest, "program");

impl Serialize for ProgramSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[serde_as]
		#[skip_serializing_none]
		#[derive(Serialize)]
		#[serde(rename = "ProgramSubscribeRequest")]
		struct Inner<'a>(
			#[serde_as(as = "DisplayFromStr")] &'a Pubkey,
			&'a Option<RpcProgramAccountsConfig>,
		);

		let inner = Inner(&self.program_id, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

impl<'de> Deserialize<'de> for ProgramSubscribeRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[serde_as]
		#[skip_serializing_none]
		#[derive(Deserialize)]
		#[serde(rename = "ProgramSubscribeRequest")]
		struct Inner(
			#[serde_as(as = "DisplayFromStr")] Pubkey,
			Option<RpcProgramAccountsConfig>,
		);

		let inner: Inner = Deserialize::deserialize(serde_tuple::Deserializer(deserializer))?;
		Ok(ProgramSubscribeRequest {
			program_id: inner.0,
			config: inner.1,
		})
	}
}

/// Which token accounts to match.
///
/// `Mint` is the faster form because the node can look accounts up by their
/// token mint directly.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTokenAccountsFilter {
	/// Match accounts holding this token mint.
	Mint(#[serde_as(as = "DisplayFromStr")] Pubkey),
	/// Match accounts owned by this token program.
	ProgramId(#[serde_as(as = "DisplayFromStr")] Pubkey),
}

/// Configuration for `getSignaturesForAddress`.
#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSignaturesForAddressConfig {
	/// Start searching backwards from this signature, as a base-58 string.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub before: Option<Signature>, // Signature as base-58 string
	/// Stop searching when this signature is reached, as a base-58 string.
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub until: Option<Signature>, // Signature as base-58 string
	/// Maximum number of signatures to return. The node caps this at 1000.
	pub limit: Option<usize>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	pub min_context_slot: Option<Slot>,
}

/// Accepts either a bare encoding or a full config object.
///
/// The deprecated form exists because these methods originally took only an
/// encoding argument before gaining a config object.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcEncodingConfigWrapper<T> {
	/// A bare encoding, as accepted by the original method signature.
	Deprecated(Option<UiTransactionEncoding>),
	/// A full config object.
	Current(Option<T>),
}

impl<T: EncodingConfig + Default + Copy> RpcEncodingConfigWrapper<T> {
	/// Resolve either form into a full config.
	pub fn convert_to_current(&self) -> T {
		match self {
			RpcEncodingConfigWrapper::Deprecated(encoding) => T::new_with_encoding(encoding),
			RpcEncodingConfigWrapper::Current(config) => config.unwrap_or_default(),
		}
	}

	/// Convert between config types, preserving the deprecated form.
	pub fn convert<U: EncodingConfig + From<T>>(&self) -> RpcEncodingConfigWrapper<U> {
		match self {
			RpcEncodingConfigWrapper::Deprecated(encoding) => {
				RpcEncodingConfigWrapper::Deprecated(*encoding)
			}
			RpcEncodingConfigWrapper::Current(config) => {
				RpcEncodingConfigWrapper::Current(config.map(Into::into))
			}
		}
	}
}

/// A config that can be built from a bare transaction encoding.
pub trait EncodingConfig {
	/// Build this config from an optional encoding.
	fn new_with_encoding(encoding: &Option<UiTransactionEncoding>) -> Self;
}

/// Configuration for `getBlock`.
///
/// <!-- {=txv1ReadHint|trim|linePrefix:"/// ":true} -->
/// The `txv1` feature gate is active on mainnet. A request that omits
/// `maxSupportedTransactionVersion` fails when it meets a v1 transaction:
/// `getTransaction` returns `-32015`, and a single v1 transaction rejects a
/// whole `getBlock`. <!-- {/txv1ReadHint} -->
#[skip_serializing_none]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockConfig {
	/// <!-- {=encodingTransaction|trim|linePrefix:"/// ":true} -->
	/// Encoding format for transaction data. Use `base64` for anything that may
	/// exceed the 1232-byte base58 limit, such as a v1 transaction.
	/// <!-- {/encodingTransaction} -->
	pub encoding: Option<UiTransactionEncoding>,
	/// <!-- {=transactionDetails|trim|linePrefix:"/// ":true} -->
	/// Level of transaction detail to return. `Full` includes transaction
	/// messages and metadata; `Accounts` returns only account keys; `None`
	/// omits transactions entirely. <!-- {/transactionDetails} -->
	pub transaction_details: Option<TransactionDetails>,
	/// Include the block's reward entries.
	pub rewards: Option<bool>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=maxSupportedTransactionVersion|trim|linePrefix:"/// ":true} -->
	/// The highest transaction version to return. Defaults to `1` so that v1
	/// transactions are readable; pass `None` to opt out.
	/// <!-- {/maxSupportedTransactionVersion} -->
	#[builder(default = Some(MAX_SUPPORTED_TRANSACTION_VERSION))]
	pub max_supported_transaction_version: Option<u8>,
}

impl Default for RpcBlockConfig {
	/// Default to reading every supported transaction version.
	fn default() -> Self {
		Self {
			encoding: None,
			transaction_details: None,
			rewards: None,
			commitment: None,
			max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
		}
	}
}

impl EncodingConfig for RpcBlockConfig {
	fn new_with_encoding(encoding: &Option<UiTransactionEncoding>) -> Self {
		Self {
			encoding: *encoding,
			..Self::default()
		}
	}
}

impl RpcBlockConfig {
	/// Fetch only the block's rewards, omitting transactions.
	pub fn rewards_only() -> Self {
		Self {
			transaction_details: Some(TransactionDetails::None),
			..Self::default()
		}
	}

	/// Fetch only the block's rewards at a specific commitment.
	pub fn rewards_with_commitment(commitment: Option<CommitmentConfig>) -> Self {
		Self {
			transaction_details: Some(TransactionDetails::None),
			commitment,
			..Self::default()
		}
	}
}

impl From<RpcBlockConfig> for RpcEncodingConfigWrapper<RpcBlockConfig> {
	fn from(config: RpcBlockConfig) -> Self {
		RpcEncodingConfigWrapper::Current(Some(config))
	}
}

/// Configuration for `getTransaction`.
///
/// <!-- {=txv1ReadHint|trim|linePrefix:"/// ":true} -->
/// The `txv1` feature gate is active on mainnet. A request that omits
/// `maxSupportedTransactionVersion` fails when it meets a v1 transaction:
/// `getTransaction` returns `-32015`, and a single v1 transaction rejects a
/// whole `getBlock`. <!-- {/txv1ReadHint} -->
#[skip_serializing_none]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionConfig {
	/// <!-- {=encodingTransaction|trim|linePrefix:"/// ":true} -->
	/// Encoding format for transaction data. Use `base64` for anything that may
	/// exceed the 1232-byte base58 limit, such as a v1 transaction.
	/// <!-- {/encodingTransaction} -->
	pub encoding: Option<UiTransactionEncoding>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=maxSupportedTransactionVersion|trim|linePrefix:"/// ":true} -->
	/// The highest transaction version to return. Defaults to `1` so that v1
	/// transactions are readable; pass `None` to opt out.
	/// <!-- {/maxSupportedTransactionVersion} -->
	#[builder(default = Some(MAX_SUPPORTED_TRANSACTION_VERSION))]
	pub max_supported_transaction_version: Option<u8>,
}

impl Default for RpcTransactionConfig {
	/// Default to reading every supported transaction version.
	fn default() -> Self {
		Self {
			encoding: None,
			commitment: None,
			max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
		}
	}
}

impl EncodingConfig for RpcTransactionConfig {
	fn new_with_encoding(encoding: &Option<UiTransactionEncoding>) -> Self {
		Self {
			encoding: *encoding,
			..Self::default()
		}
	}
}

/// Either an end slot or a commitment, depending on which form the caller used.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcBlocksConfigWrapper {
	/// Limit the range with an end slot.
	EndSlotOnly(Option<Slot>),
	/// Select the range with a commitment level alone.
	CommitmentOnly(Option<CommitmentConfig>),
}

impl RpcBlocksConfigWrapper {
	/// Split into the end slot and commitment halves.
	pub fn unzip(&self) -> (Option<Slot>, Option<CommitmentConfig>) {
		match &self {
			RpcBlocksConfigWrapper::EndSlotOnly(end_slot) => (*end_slot, None),
			RpcBlocksConfigWrapper::CommitmentOnly(commitment) => (None, *commitment),
		}
	}
}

/// Commitment and minimum slot for methods that return a value with a context.
#[skip_serializing_none]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcContextConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=minContextSlot|trim|linePrefix:"/// ":true} -->
	/// The minimum slot that the request can be evaluated at. Enforced only
	/// when the commitment is `processed`. <!-- {/minContextSlot} -->
	pub min_context_slot: Option<Slot>,
}

/// Configuration for the deprecated
/// `getConfirmedSignaturesForAddress2` method.
#[derive(Debug, Default)]
pub struct GetConfirmedSignaturesForAddress2Config {
	/// Start searching backwards from this signature.
	pub before: Option<Signature>,
	/// Stop searching when this signature is reached.
	pub until: Option<Signature>,
	/// Maximum number of signatures to return.
	pub limit: Option<usize>,
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	pub commitment: Option<CommitmentConfig>,
}

/// Configuration for `logsSubscribe`.
#[skip_serializing_none]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionLogsConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

/// Which transaction logs to receive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTransactionLogsFilter {
	/// All transactions, excluding vote transactions.
	All,
	/// All transactions, including votes.
	AllWithVotes,
	/// Only transactions that mention one of these addresses, given as
	/// base-58 encoded strings.
	Mentions(Vec<String>), // base58-encoded list of addresses
}

/// Configuration for `accountSubscribe`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcAccountSubscribeConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=encodingAccount|trim|linePrefix:"/// ":true} -->
	/// Encoding format for account data.
	/// <!-- {/encodingAccount} -->
	pub encoding: Option<UiTransactionEncoding>,
}

/// Configuration for `blockSubscribe`.
///
/// <!-- {=txv1ReadHint|trim|linePrefix:"/// ":true} -->
/// The `txv1` feature gate is active on mainnet. A request that omits
/// `maxSupportedTransactionVersion` fails when it meets a v1 transaction:
/// `getTransaction` returns `-32015`, and a single v1 transaction rejects a
/// whole `getBlock`. <!-- {/txv1ReadHint} -->
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockSubscribeConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// <!-- {=encodingTransaction|trim|linePrefix:"/// ":true} -->
	/// Encoding format for transaction data. Use `base64` for anything that may
	/// exceed the 1232-byte base58 limit, such as a v1 transaction.
	/// <!-- {/encodingTransaction} -->
	pub encoding: Option<UiTransactionEncoding>,
	/// <!-- {=transactionDetails|trim|linePrefix:"/// ":true} -->
	/// Level of transaction detail to return. `Full` includes transaction
	/// messages and metadata; `Accounts` returns only account keys; `None`
	/// omits transactions entirely. <!-- {/transactionDetails} -->
	pub transaction_details: Option<TransactionDetails>,
	/// Include the block's reward entries.
	pub show_rewards: Option<bool>,
	/// <!-- {=maxSupportedTransactionVersion|trim|linePrefix:"/// ":true} -->
	/// The highest transaction version to return. Defaults to `1` so that v1
	/// transactions are readable; pass `None` to opt out.
	/// <!-- {/maxSupportedTransactionVersion} -->
	#[builder(default = Some(MAX_SUPPORTED_TRANSACTION_VERSION))]
	pub max_supported_transaction_version: Option<u8>,
}

impl Default for RpcBlockSubscribeConfig {
	/// Default to reading every supported transaction version.
	fn default() -> Self {
		Self {
			commitment: None,
			encoding: None,
			transaction_details: None,
			show_rewards: None,
			max_supported_transaction_version: Some(MAX_SUPPORTED_TRANSACTION_VERSION),
		}
	}
}

/// Which blocks to receive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcBlockSubscribeFilter {
	/// All blocks.
	All,
	/// Only blocks that mention the given account or program.
	MentionsAccountOrProgram(String),
}

/// Configuration for `signatureSubscribe`.
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcSignatureSubscribeConfig {
	/// <!-- {=commitmentField|trim|linePrefix:"/// ":true} -->
	/// Commitment level for the request. When omitted the field is left out of
	/// the payload, so the node applies its own default. The client's
	/// commitment is only applied by the convenience methods that build a
	/// config from it, such as `get_balance`. <!-- {/commitmentField} -->
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	/// Also emit a notification when the signature is first received, before it
	/// is processed.
	pub enable_received_notification: Option<bool>,
}

/// A `blockSubscribe` request.
#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct BlockSubscribeRequest {
	/// Which blocks to receive.
	pub filter: RpcBlockSubscribeFilter,
	/// Encoding and detail options for the returned blocks.
	#[builder(default, setter(into, strip_option(fallback = config_opt)))]
	pub config: Option<RpcBlockSubscribeConfig>,
}

impl_websocket_method!(BlockSubscribeRequest, "block");

impl Serialize for BlockSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[skip_serializing_none]
		#[derive(serde::Serialize)]
		#[serde(rename = "BlockSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(
			&'serde_tuple_inner RpcBlockSubscribeFilter,
			&'serde_tuple_inner Option<RpcBlockSubscribeConfig>,
		);

		let inner = Inner(&self.filter, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

impl<'de> Deserialize<'de> for BlockSubscribeRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[skip_serializing_none]
		#[derive(serde::Deserialize)]
		#[serde(rename = "BlockSubscribeRequest")]
		struct Inner(RpcBlockSubscribeFilter, Option<RpcBlockSubscribeConfig>);

		let inner: Inner = Deserialize::deserialize(serde_tuple::Deserializer(deserializer))?;
		Ok(BlockSubscribeRequest {
			filter: inner.0,
			config: inner.1,
		})
	}
}

/// A `logsSubscribe` request.
#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct LogsSubscribeRequest {
	/// Which transaction logs to receive.
	pub filter: RpcTransactionLogsFilter,
	/// Commitment level for the subscription.
	#[builder(default, setter(into))]
	pub config: RpcTransactionLogsConfig,
}

impl_websocket_method!(LogsSubscribeRequest, "logs");

impl Serialize for LogsSubscribeRequest {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		#[derive(Serialize)]
		#[serde(rename = "LogsSubscribeRequest")]
		struct Inner<'serde_tuple_inner>(
			&'serde_tuple_inner RpcTransactionLogsFilter,
			&'serde_tuple_inner RpcTransactionLogsConfig,
		);

		let inner = Inner(&self.filter, &self.config);
		Serialize::serialize(&inner, serde_tuple::Serializer(serializer))
	}
}

impl<'de> Deserialize<'de> for LogsSubscribeRequest {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		#[serde(rename = "LogsSubscribeRequest")]
		struct Inner(RpcTransactionLogsFilter, RpcTransactionLogsConfig);

		let inner: Inner = Deserialize::deserialize(serde_tuple::Deserializer(deserializer))?;
		Ok(LogsSubscribeRequest {
			filter: inner.0,
			config: inner.1,
		})
	}
}
