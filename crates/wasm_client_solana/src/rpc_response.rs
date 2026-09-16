//! Response types for the Solana JSON-RPC and `WebSocket` `PubSub` methods
//! exposed by this crate.
//!
//! The shapes mirror what the node returns over the wire; serde attributes
//! handle base58/base64 encodings and camelCase or kebab-case field names.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::result::Result;
use std::str::FromStr;

use derive_more::derive::Deref;
use derive_more::derive::DerefMut;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_clock::Epoch;
use solana_clock::Slot;
use solana_clock::UnixTimestamp;
use solana_fee_calculator::FeeCalculator;
use solana_fee_calculator::FeeRateGovernor;
use solana_hash::Hash;
use solana_inflation::Inflation;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction_error::TransactionError;
use solana_transaction_error::TransactionResult;
use thiserror::Error;

use crate::Context;
use crate::impl_websocket_notification;
use crate::solana_account_decoder::UiAccount;
use crate::solana_account_decoder::parse_token::UiTokenAmount;
use crate::solana_transaction_status::ConfirmedTransactionStatusWithSignature;
use crate::solana_transaction_status::TransactionConfirmationStatus;
use crate::solana_transaction_status::UiConfirmedBlock;
use crate::solana_transaction_status::UiInnerInstructions;
use crate::solana_transaction_status::UiTransactionReturnData;

/// Wrapper for rpc return types of methods that provide responses both with and
/// without context. Main purpose of this is to fix methods that lack context
/// information in their return type, without breaking backwards compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OptionalContext<T> {
	/// Response value accompanied by the slot context it was evaluated at.
	Context(Response<T>),
	/// Response value returned without context, as by legacy methods.
	NoContext(T),
}

impl<T> OptionalContext<T> {
	/// Unwraps the response into its value, discarding any context.
	pub fn parse_value(self) -> T {
		match self {
			Self::Context(response) => response.value,
			Self::NoContext(value) => value,
		}
	}
}

/// Context attached to a JSON-RPC response, identifying where the node was in
/// the ledger when it produced the answer.
#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcResponseContext {
	/// The slot the node had processed when the response was generated.
	pub slot: Slot,
	/// Version of the node software that produced the response, if reported.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub api_version: Option<RpcApiVersion>,
}

/// Version of the RPC API reported by the node, such as `1.18.26`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcApiVersion(semver::Version);

impl std::ops::Deref for RpcApiVersion {
	type Target = semver::Version;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Default for RpcApiVersion {
	fn default() -> Self {
		Self(solana_version::Version::default().as_semver_version())
	}
}

impl Serialize for RpcApiVersion {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

impl<'de> Deserialize<'de> for RpcApiVersion {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s: String = Deserialize::deserialize(deserializer)?;
		Ok(RpcApiVersion(
			semver::Version::from_str(&s).map_err(serde::de::Error::custom)?,
		))
	}
}

impl RpcResponseContext {
	/// Creates a context for `slot`, stamped with the version of the Solana
	/// dependency this crate was built against.
	pub fn new(slot: Slot) -> Self {
		Self {
			slot,
			api_version: Some(RpcApiVersion::default()),
		}
	}
}

/// A JSON-RPC response value paired with the slot context it was evaluated at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response<T> {
	/// Ledger context the node used to serve the value.
	pub context: RpcResponseContext,
	/// The response payload itself.
	pub value: T,
}

/// Commitment of a block by its hash, including the stake that has voted on it.
#[skip_serializing_none]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockCommitment<T> {
	/// Commitment information for the block, or `None` if the block is unknown.
	pub commitment: Option<T>,
	/// Total active stake, in lamports, that has voted on the block.
	pub total_stake: u64,
}

/// A recent blockhash together with the fee schedule that applied to it.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockhashFeeCalculator {
	/// Blockhash as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Fee schedule in effect for the blockhash.
	pub fee_calculator: FeeCalculator,
}

/// A recent blockhash and the block height at which it expires.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockhash {
	/// Blockhash as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Last block height at which the blockhash is still valid; transactions
	/// referencing it expire at the next block.
	pub last_valid_block_height: u64,
}

/// Fee information for a blockhash, including the slot and block height at
/// which it expires.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcFees {
	/// Blockhash as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Fee schedule in effect for the blockhash.
	pub fee_calculator: FeeCalculator,
	/// Slot at which the blockhash expires.
	pub last_valid_slot: Slot,
	/// Last block height at which the blockhash is still valid.
	pub last_valid_block_height: u64,
}

/// Legacy fee response predating `last_valid_block_height`; retained for
/// deserializing responses from older nodes.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeprecatedRpcFees {
	/// Blockhash as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Fee schedule in effect for the blockhash.
	pub fee_calculator: FeeCalculator,
	/// Slot at which the blockhash expires.
	pub last_valid_slot: Slot,
}

/// Fee information for a recent blockhash, keyed by expiry block height.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fees {
	/// Blockhash as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Fee schedule in effect for the blockhash.
	pub fee_calculator: FeeCalculator,
	/// Last block height at which the blockhash is still valid.
	pub last_valid_block_height: u64,
}

/// Wrapper around the fee schedule for a recent blockhash.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcFeeCalculator {
	/// Fee schedule in effect for the queried blockhash.
	pub fee_calculator: FeeCalculator,
}

/// Wrapper around the fee rate governor, which governs how transaction fees
/// adjust with network congestion.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcFeeRateGovernor {
	/// Current fee rate governor parameters.
	pub fee_rate_governor: FeeRateGovernor,
}

/// Parameters of the inflation schedule: the rate at which new SOL is issued
/// and how it tapers over time.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcInflationGovernor {
	/// Initial inflation rate, as a fraction of total supply per year.
	pub initial: f64,
	/// Terminal inflation rate that the schedule tapers toward.
	pub terminal: f64,
	/// Fraction by which the inflation rate is reduced each epoch.
	pub taper: f64,
	/// Percentage of inflation allocated to the foundation.
	pub foundation: f64,
	/// Number of years over which foundation inflation is distributed.
	pub foundation_term: f64,
}

impl PartialEq for RpcInflationGovernor {
	fn eq(&self, other: &Self) -> bool {
		approx_eq(self.initial, other.initial)
			&& approx_eq(self.terminal, other.terminal)
			&& approx_eq(self.taper, other.taper)
			&& approx_eq(self.foundation, other.foundation)
			&& approx_eq(self.foundation_term, other.foundation_term)
	}
}

impl Eq for RpcInflationGovernor {}

/// Compares two floats for approximate equality, treating differences below
/// `1e-6` as equal.
pub fn approx_eq(a: f64, b: f64) -> bool {
	const EPSILON: f64 = 1e-6;
	(a - b).abs() < EPSILON
}

impl From<Inflation> for RpcInflationGovernor {
	fn from(inflation: Inflation) -> Self {
		Self {
			initial: inflation.initial,
			terminal: inflation.terminal,
			taper: inflation.taper,
			foundation: inflation.foundation,
			foundation_term: inflation.foundation_term,
		}
	}
}

/// Current inflation rate, broken down by the share going to validators and to
/// the foundation.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RpcInflationRate {
	/// Total annual inflation rate as a fraction of the money supply.
	pub total: f64,
	/// Share of `total` distributed to validators.
	pub validator: f64,
	/// Share of `total` distributed to the foundation.
	pub foundation: f64,
	/// Epoch the rate applies to.
	pub epoch: Epoch,
}

/// An account returned by a query that pairs it with the pubkey it is stored
/// under.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcKeyedAccount {
	/// Account address as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	/// Account data, in the encoding requested by the caller.
	pub account: UiAccount,
}

/// Position of a slot relative to its parent and to the node's current root.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotInfo {
	/// Slot these relationships describe.
	pub slot: Slot,
	/// Parent of `slot` in the ledger.
	pub parent: Slot,
	/// Root of the node's bank at the time the update was sent.
	pub root: Slot,
}

/// Transaction counts recorded while a slot was being produced.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlotTransactionStats {
	/// Number of entries processed in the slot.
	pub num_transaction_entries: u64,
	/// Number of transactions that executed successfully.
	pub num_successful_transactions: u64,
	/// Number of transactions that failed.
	pub num_failed_transactions: u64,
	/// Largest number of transactions packed into a single entry.
	pub max_transactions_per_entry: u64,
}

/// Progress update for a slot, streamed by the `slotUpdates` subscription as
/// the slot advances toward confirmation.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SlotUpdate {
	/// The first shred for the slot has been received from the network.
	FirstShredReceived {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
	},
	/// All shreds for the slot have been received, so the block is complete.
	Completed {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
	},
	/// A bank has been created for the slot and is ready to replay.
	CreatedBank {
		/// Slot this update refers to.
		slot: Slot,
		/// Parent slot of the newly created bank.
		parent: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
	},
	/// The bank for the slot has been frozen and can no longer change.
	Frozen {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
		/// Transaction counts recorded while the slot was produced.
		stats: SlotTransactionStats,
	},
	/// The slot was abandoned by the cluster, for example after a fork was
	/// pruned.
	Dead {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
		/// Description of why the slot died.
		err: String,
	},
	/// The slot has been optimistically confirmed by a supermajority of stake.
	OptimisticConfirmation {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
	},
	/// The slot has been rooted and will never be rolled back.
	Root {
		/// Slot this update refers to.
		slot: Slot,
		/// Milliseconds since the Unix epoch when the update was emitted.
		timestamp: u64,
	},
}

impl SlotUpdate {
	/// Returns the slot this update describes, regardless of variant.
	pub fn slot(&self) -> Slot {
		match self {
			Self::FirstShredReceived { slot, .. }
			| Self::Completed { slot, .. }
			| Self::CreatedBank { slot, .. }
			| Self::Frozen { slot, .. }
			| Self::Dead { slot, .. }
			| Self::OptimisticConfirmation { slot, .. }
			| Self::Root { slot, .. } => *slot,
		}
	}
}

/// Result of a signature subscription, reported either as a processed
/// transaction result or as a plain receipt acknowledgment.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum RpcSignatureResult {
	/// The transaction with this signature was processed, successfully or not.
	ProcessedSignature(ProcessedSignatureResult),
	/// The node received the transaction with this signature.
	ReceivedSignature(ReceivedSignatureResult),
}

/// Log messages and error, if any, produced by a single transaction.
#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcLogsResponse {
	/// Transaction signature as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub signature: Signature, // Signature as base58 string
	/// Error the transaction failed with, or `None` if it succeeded.
	pub err: Option<TransactionError>,
	/// Program log lines emitted while the transaction executed.
	pub logs: Vec<String>,
}

/// Payload of a `logsSubscribe` notification, carrying the log response and
/// the slot context it was produced at.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct LogsNotificationResponse {
	/// Slot context for the logs.
	pub context: Context,
	/// Logs emitted by the subscribed transaction.
	pub value: RpcLogsResponse,
}

impl_websocket_notification!(LogsNotificationResponse, "logs");

/// Result of processing a transaction, carrying the error it failed with if
/// it was not successful.
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedSignatureResult {
	/// Error the transaction failed with, or `None` if it succeeded.
	pub err: Option<TransactionError>,
}

/// Confirmation that the node has received a transaction, with no outcome
/// information.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReceivedSignatureResult {
	/// The transaction signature was received by the node.
	ReceivedSignature,
}

/// Network addresses and version information exposed by a cluster node's
/// gossip and `TPU` ports.
#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcContactInfo {
	/// Pubkey of the node as a base-58 string
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	/// Gossip port
	pub gossip: Option<SocketAddr>,
	/// Tvu UDP port
	pub tvu: Option<SocketAddr>,
	/// Tpu UDP port
	pub tpu: Option<SocketAddr>,
	/// Tpu QUIC port
	pub tpu_quic: Option<SocketAddr>,
	/// Tpu UDP forwards port
	pub tpu_forwards: Option<SocketAddr>,
	/// Tpu QUIC forwards port
	pub tpu_forwards_quic: Option<SocketAddr>,
	/// Tpu UDP vote port
	pub tpu_vote: Option<SocketAddr>,
	/// Server repair UDP port
	pub serve_repair: Option<SocketAddr>,
	/// JSON RPC port
	pub rpc: Option<SocketAddr>,
	/// `WebSocket` `PubSub` port
	pub pubsub: Option<SocketAddr>,
	/// Software version
	pub version: Option<String>,
	/// First 4 bytes of the `FeatureSet` identifier
	pub feature_set: Option<u32>,
	/// Shred version
	pub shred_version: Option<u16>,
}

/// Map of leader base58 identity pubkeys to the slot indices relative to the
/// first epoch slot
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, Deref, DerefMut)]
pub struct RpcLeaderSchedule(#[serde(with = "pubkey_string_map")] pub HashMap<Pubkey, Vec<usize>>);

mod pubkey_string_map {
	use std::result::Result;

	use serde::ser::SerializeMap;

	use super::*;

	pub fn serialize<S>(map: &HashMap<Pubkey, Vec<usize>>, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut ser_map = serializer.serialize_map(Some(map.len()))?;
		for (k, v) in map {
			ser_map.serialize_entry(&k.to_string(), v)?;
		}
		ser_map.end()
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<HashMap<Pubkey, Vec<usize>>, D::Error>
	where
		D: Deserializer<'de>,
	{
		let string_map: HashMap<String, Vec<usize>> = HashMap::deserialize(deserializer)?;
		string_map
			.into_iter()
			.map(|(k, v)| Ok((Pubkey::from_str(&k).map_err(serde::de::Error::custom)?, v)))
			.collect()
	}
}

/// Inclusive slot range a block production query covered.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProductionRange {
	/// First slot of the range.
	pub first_slot: Slot,
	/// Last slot of the range.
	pub last_slot: Slot,
}

/// Block production totals for a cluster over a slot range.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProduction {
	/// Map of leader base58 identity pubkeys to a tuple of `(number of leader
	/// slots, number of blocks produced)`
	pub by_identity: HashMap<String, (usize, usize)>,
	/// Slot range the totals cover.
	pub range: RpcBlockProductionRange,
}

/// Version information for a node: its `solana-core` release and feature set
/// identifier.
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct RpcVersionInfo {
	/// The current version of solana-core
	pub solana_core: String,
	/// first 4 bytes of the `FeatureSet` identifier
	pub feature_set: Option<u32>,
}

impl fmt::Debug for RpcVersionInfo {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.solana_core)
	}
}

impl fmt::Display for RpcVersionInfo {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if let Some(version) = self.solana_core.split_whitespace().next() {
			// Display just the semver if possible
			write!(f, "{version}")
		} else {
			write!(f, "{}", self.solana_core)
		}
	}
}

/// The identity pubkey that a node uses to sign votes and identify itself in
/// gossip.
#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct RpcIdentity {
	/// The current node identity pubkey
	#[serde_as(as = "DisplayFromStr")]
	pub identity: Pubkey,
}

/// A vote cast by a validator together with the hash it voted for.
#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcVote {
	/// Vote account address, as base-58 encoded string
	#[serde_as(as = "DisplayFromStr")]
	pub vote_pubkey: Pubkey,
	/// Slots the vote confirms.
	pub slots: Vec<Slot>,
	/// Hash of the voted-for bank, as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub hash: Hash,
	/// Wall-clock time, as seconds since the Unix epoch, when the vote was
	/// cast.
	pub timestamp: Option<UnixTimestamp>,
	/// Signature over the vote, as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub signature: Signature,
}

/// Vote accounts of a cluster grouped into current and delinquent sets.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcVoteAccountStatus {
	/// Vote accounts that are participating in the current epoch.
	pub current: Vec<RpcVoteAccountInfo>,
	/// Vote accounts that have stopped voting and are delinquent.
	pub delinquent: Vec<RpcVoteAccountInfo>,
}

/// Summary of a vote account and the stake delegated to it.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcVoteAccountInfo {
	/// Vote account address, as base-58 encoded string
	#[serde_as(as = "DisplayFromStr")]
	pub vote_pubkey: Pubkey,
	/// The validator identity, as base-58 encoded string
	#[serde_as(as = "DisplayFromStr")]
	pub node_pubkey: Pubkey,
	/// The current stake, in lamports, delegated to this vote account
	pub activated_stake: u64,
	/// An 8-bit integer used as a fraction (`commission/MAX_U8`) for rewards
	/// payout
	pub commission: u8,
	/// Whether this account is staked for the current epoch
	pub epoch_vote_account: bool,
	/// Latest history of earned credits for up to
	/// `MAX_RPC_VOTE_ACCOUNT_INFO_EPOCH_CREDITS_HISTORY` epochs   each tuple
	/// is (Epoch, credits, `prev_credits`)
	pub epoch_credits: Vec<(Epoch, u64, u64)>,
	/// Most recent slot voted on by this vote account (0 if no votes exist)
	#[serde(default)]
	pub last_vote: u64,

	/// Current root slot for this vote account (0 if no root slot exists)
	#[serde(default)]
	pub root_slot: Slot,
}

/// Confirmation status of a transaction signature, as returned by
/// `getSignatureStatuses`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureConfirmation {
	/// Number of confirmations the transaction has received at the requested
	/// commitment level.
	pub confirmations: usize,
	/// Execution result of the transaction.
	pub status: TransactionResult<()>,
}

/// Outcome of simulating a transaction, including logs and account state
/// changes produced by the simulation.
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionResult {
	/// Error the transaction would fail with, or `None` if it would succeed.
	pub err: Option<TransactionError>,
	/// Program log lines emitted during simulation.
	pub logs: Option<Vec<String>>,
	/// Accounts that the transaction would load, when account state was
	/// requested.
	pub accounts: Option<Vec<Option<UiAccount>>>,
	/// Compute units the transaction would consume.
	pub units_consumed: Option<u64>,
	/// Data returned by the transaction's programs.
	pub return_data: Option<UiTransactionReturnData>,
	/// Inner instructions executed by the transaction's programs.
	pub inner_instructions: Option<Vec<UiInnerInstructions>>,
	/// If the blockhash used was invalid, a replacement blockhash and its
	/// expiry height to retry with.
	pub replacement_blockhash: Option<RpcBlockhash>,
}

/// A blockhash recorded by the storage turn query, together with the slot it
/// was produced in.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcStorageTurn {
	/// Blockhash of the turn, as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub blockhash: Hash,
	/// Slot the blockhash was produced in.
	pub slot: Slot,
}

/// An account address and the lamport balance held by it.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcAccountBalance {
	/// Account address as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub address: Pubkey,
	/// Balance of the account, in lamports.
	pub lamports: u64,
}

/// Snapshot of the cluster's SOL supply split by circulation status, from
/// `getSupply`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcSupply {
	/// Total supply, in lamports.
	pub total: u64,
	/// Portion of the supply counted as circulating, in lamports.
	pub circulating: u64,
	/// Portion of the supply excluded from circulation, in lamports.
	pub non_circulating: u64,
	/// Addresses of the accounts holding the non-circulating supply, as
	/// base58-encoded strings.
	pub non_circulating_accounts: Vec<String>,
}

/// Stage of the stake activation or deactivation process that a stake account
/// is in.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StakeActivationState {
	/// The stake is warming up and is not yet fully active.
	Activating,
	/// The stake is fully active and earning rewards.
	Active,
	/// The stake is cooling down and no longer fully active.
	Deactivating,
	/// The stake is fully inactive and earns no rewards.
	Inactive,
}

/// Activation status and amounts for a stake account, from
/// `getStakeActivation`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcStakeActivation {
	/// Current activation stage of the stake.
	pub state: StakeActivationState,
	/// Amount of stake that is active, in lamports.
	pub active: u64,
	/// Amount of stake that is inactive, in lamports.
	pub inactive: u64,
}

/// Balance of a single token account, flattened together with its address.
#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RpcTokenAccountBalance {
	/// Token account address as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub address: Pubkey,
	/// Raw and UI amounts held by the token account.
	#[serde(flatten)]
	pub amount: UiTokenAmount,
}

/// A transaction signature with its status, as returned by
/// `getSignaturesForAddress`.
#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcConfirmedTransactionStatusWithSignature {
	/// Transaction signature as a base58-encoded string.
	#[serde_as(as = "DisplayFromStr")]
	pub signature: Signature,
	/// Slot the transaction was processed in.
	pub slot: Slot,
	/// Error the transaction failed with, or `None` if it succeeded.
	pub err: Option<TransactionError>,
	/// Memo attached to the transaction, if any.
	pub memo: Option<String>,
	/// Estimated production time of the block as seconds since the Unix
	/// epoch, if available.
	pub block_time: Option<UnixTimestamp>,
	/// The transaction index within the block.
	pub index: u32,
	/// Commitment level at which the node reported the transaction, if known.
	pub confirmation_status: Option<TransactionConfirmationStatus>,
}

/// Performance sample for a node over a short window, from
/// `getRecentPerformanceSamples`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcPerfSample {
	/// Slot the sample was taken at.
	pub slot: Slot,
	/// Total transactions processed in the sample period.
	pub num_transactions: u64,
	/// Transactions processed in the sample period that were not simple vote
	/// transactions.
	pub num_non_vote_transaction: u64,
	/// Number of slots in the sample period.
	pub num_slots: u64,
	/// Length of the sample period, in seconds.
	pub sample_period_secs: u16,
}

impl RpcPerfSample {
	/// Number of vote transactions processed in the sample period, derived by
	/// subtracting non-vote transactions from the total.
	pub fn num_vote_transactions(&self) -> u64 {
		self.num_transactions - self.num_non_vote_transaction
	}
}

/// Inflation reward credited to a stake or vote account in a single epoch, from
/// `getInflationReward`.
#[skip_serializing_none]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcInflationReward {
	/// Epoch the reward was distributed for.
	pub epoch: Epoch,
	/// Slot the reward was credited at.
	pub effective_slot: Slot,
	/// Reward amount, in lamports.
	pub amount: u64, // lamports
	/// Account balance after the reward was credited, in lamports.
	pub post_balance: u64, // lamports
	/// Vote account commission when the reward was credited, as a percentage.
	pub commission: Option<u8>, // Vote account commission when the reward was credited
}

/// Reason a slot update could not be delivered over the block subscription.
#[derive(Clone, Copy, Deserialize, Serialize, Debug, Error, Eq, PartialEq)]
pub enum RpcBlockUpdateError {
	/// The node's block store failed to produce the slot's block.
	#[error("block store error")]
	BlockStoreError,

	/// The block contains a transaction version the client did not opt into.
	#[error("unsupported transaction version ({0})")]
	UnsupportedTransactionVersion(u8),
}

/// Newly produced block, or the error that prevented it from being retrieved.
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockUpdate {
	/// Slot the block was produced in.
	pub slot: Slot,
	/// The block, when it could be retrieved.
	pub block: Option<UiConfirmedBlock>,
	/// Why the block could not be returned, when retrieval failed.
	pub err: Option<RpcBlockUpdateError>,
}

/// Payload of a `blockSubscribe` notification, carrying the block update and
/// the slot context it was produced at.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct BlockNotificationResponse {
	/// Slot context for the block update.
	pub context: Context,
	/// The block update or the error that replaced it.
	pub value: RpcBlockUpdate,
}

impl_websocket_notification!(BlockNotificationResponse, "block");

impl From<ConfirmedTransactionStatusWithSignature> for RpcConfirmedTransactionStatusWithSignature {
	fn from(value: ConfirmedTransactionStatusWithSignature) -> Self {
		let ConfirmedTransactionStatusWithSignature {
			signature,
			slot,
			err,
			memo,
			block_time,
			index,
		} = value;
		Self {
			signature,
			slot,
			err,
			memo,
			block_time,
			index,
			confirmation_status: None,
		}
	}
}

/// Slots covered by the most recent full and incremental snapshots.
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RpcSnapshotSlotInfo {
	/// Slot of the most recent full snapshot.
	pub full: Slot,
	/// Slot of the most recent incremental snapshot, if one exists.
	pub incremental: Option<Slot>,
}

/// Prioritization fee observed by the node in a given slot, from
/// `getRecentPrioritizationFees`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RpcPrioritizationFee {
	/// Slot the fee was observed in.
	pub slot: Slot,
	/// Highest per-compute-unit prioritization fee, in micro-lamports, paid
	/// by a transaction in the slot.
	pub prioritization_fee: u64,
}
