use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bincode::serialize;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::DeserializeOwned;
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

use super::rpc_filter::RpcFilterType;
use crate::ClientError;
use crate::ClientResult;
use crate::RpcError;
use crate::SolanaRpcClient;
use crate::impl_websocket_method;
use crate::nonce_utils;
use crate::solana_account_decoder::UiAccount;
use crate::solana_account_decoder::UiAccountEncoding;
use crate::solana_account_decoder::UiDataSliceConfig;
use crate::solana_transaction_status::TransactionDetails;
use crate::solana_transaction_status::UiTransactionEncoding;

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcKeyedAccount {
	#[serde_as(as = "DisplayFromStr")]
	pub pubkey: Pubkey,
	pub account: UiAccount,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Source {
	Cluster,
	NonceAccount(Pubkey),
}

impl Source {
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

#[derive(Debug, PartialEq, Eq)]
pub enum BlockhashQuery {
	None(Hash),
	FeeCalculator(Source, Hash),
	All(Source),
}

impl BlockhashQuery {
	pub fn new(blockhash: Option<Hash>, sign_only: bool, nonce_account: Option<Pubkey>) -> Self {
		let source = nonce_account.map_or(Source::Cluster, Source::NonceAccount);
		match blockhash {
			Some(hash) if sign_only => Self::None(hash),
			Some(hash) if !sign_only => Self::FeeCalculator(source, hash),
			None if !sign_only => Self::All(source),
			_ => panic!("Cannot resolve blockhash"),
		}
	}

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
	fn default() -> Self {
		BlockhashQuery::All(Source::Cluster)
	}
}

pub fn serialize_and_encode<T>(input: &T, encoding: UiTransactionEncoding) -> ClientResult<String>
where
	T: Serialize,
{
	let serialized =
		serialize(input).map_err(|e| RpcError::new(format!("Serialization failed: {e}")))?;
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

pub fn deserialize_and_decode<T: DeserializeOwned>(
	content: &str,
	encoding: UiTransactionEncoding,
) -> ClientResult<T> {
	let decoded = match encoding {
		UiTransactionEncoding::Base64 => {
			bincode::deserialize(
				&BASE64_STANDARD
					.decode(content)
					.map_err(|e| ClientError::Other(e.to_string()))?,
			)
			.map_err(|e| ClientError::Other(e.to_string()))?
		}
		UiTransactionEncoding::Base58 => {
			bincode::deserialize(
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSignatureStatusConfig {
	pub search_transaction_history: bool,
}

#[skip_serializing_none]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSendTransactionConfig {
	#[serde(default)]
	#[builder(!default, setter(into, !strip_option, strip_bool(fallback = skip_preflight_bool)))]
	pub skip_preflight: bool,
	pub preflight_commitment: Option<CommitmentLevel>,
	pub encoding: Option<UiTransactionEncoding>,
	pub max_retries: Option<usize>,
	pub min_context_slot: Option<Slot>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionAccountsConfig {
	pub encoding: Option<UiAccountEncoding>,
	#[builder(setter(!strip_option))]
	pub addresses: Vec<String>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcSimulateTransactionConfig {
	#[serde(default)]
	#[builder(setter(into, strip_bool(fallback = sig_verify_bool)))]
	pub sig_verify: bool,
	#[serde(default)]
	#[builder(default, setter(into, strip_option(fallback = replace_recent_blockhash_opt)))]
	pub replace_recent_blockhash: Option<bool>,
	#[serde(flatten)]
	#[builder(default, setter(into, strip_option(fallback = commitment_opt)))]
	pub commitment: Option<CommitmentConfig>,
	#[builder(default, setter(into, strip_option(fallback = encoding_opt)))]
	pub encoding: Option<UiTransactionEncoding>,
	#[builder(default, setter(into, strip_option(fallback = accounts_opt)))]
	pub accounts: Option<RpcSimulateTransactionAccountsConfig>,
	#[builder(default, setter(into, strip_option(fallback = min_context_slot_opt)))]
	pub min_context_slot: Option<Slot>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcRequestAirdropConfig {
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub recent_blockhash: Option<Hash>, // base-58 encoded blockhash
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcLeaderScheduleConfig {
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub identity: Option<Pubkey>, // validator identity, as a base-58 encoded string
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProductionConfigRange {
	pub first_slot: Slot,
	#[builder(default, setter(into, strip_option(fallback = last_slot_opt)))]
	pub last_slot: Option<Slot>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockProductionConfig {
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub identity: Option<Pubkey>, // validator identity, as a base-58 encoded string
	pub range: Option<RpcBlockProductionConfigRange>, // current epoch if `None`
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcGetVoteAccountsConfig {
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub vote_pubkey: Option<Pubkey>, // validator vote address, as a base-58 encoded string
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub keep_unstaked_delinquents: Option<bool>,
	pub delinquent_slot_distance: Option<u64>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcLeaderScheduleConfigWrapper {
	SlotOnly(Option<Slot>),
	ConfigOnly(Option<RpcLeaderScheduleConfig>),
}

impl RpcLeaderScheduleConfigWrapper {
	pub fn unzip(&self) -> (Option<Slot>, Option<RpcLeaderScheduleConfig>) {
		match &self {
			RpcLeaderScheduleConfigWrapper::SlotOnly(slot) => (*slot, None),
			RpcLeaderScheduleConfigWrapper::ConfigOnly(config) => (None, config.clone()),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcLargestAccountsFilter {
	Circulating,
	NonCirculating,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcLargestAccountsConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub filter: Option<RpcLargestAccountsFilter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSupplyConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	#[serde(default)]
	pub exclude_non_circulating_accounts_list: bool,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcEpochConfig {
	pub epoch: Option<Epoch>,
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub min_context_slot: Option<Slot>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcAccountInfoConfig {
	#[builder(default = Some(UiAccountEncoding::Base64), setter(into, strip_option(fallback = encoding_opt)))]
	pub encoding: Option<UiAccountEncoding>,
	#[builder(default, setter(into, strip_option(fallback = data_slice_opt)))]
	pub data_slice: Option<UiDataSliceConfig>,
	#[serde(flatten)]
	#[builder(default, setter(into, strip_option(fallback = commitment_opt)))]
	pub commitment: Option<CommitmentConfig>,
	#[builder(default, setter(into, strip_option(fallback = min_context_slot_opt)))]
	pub min_context_slot: Option<Slot>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct RpcProgramAccountsConfig {
	#[builder(default, setter(strip_option(fallback = filters_opt)))]
	pub filters: Option<Vec<RpcFilterType>>,
	#[serde(flatten)]
	pub account_config: RpcAccountInfoConfig,
	#[builder(default, setter(strip_option(fallback = with_context_opt)))]
	pub with_context: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, TypedBuilder)]
pub struct ProgramSubscribeRequest {
	pub program_id: Pubkey,
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

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTokenAccountsFilter {
	Mint(#[serde_as(as = "DisplayFromStr")] Pubkey),
	ProgramId(#[serde_as(as = "DisplayFromStr")] Pubkey),
}

#[serde_as]
#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcSignaturesForAddressConfig {
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub before: Option<Signature>, // Signature as base-58 string
	#[serde_as(as = "Option<DisplayFromStr>")]
	pub until: Option<Signature>, // Signature as base-58 string
	pub limit: Option<usize>,
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub min_context_slot: Option<Slot>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcEncodingConfigWrapper<T> {
	Deprecated(Option<UiTransactionEncoding>),
	Current(Option<T>),
}

impl<T: EncodingConfig + Default + Copy> RpcEncodingConfigWrapper<T> {
	pub fn convert_to_current(&self) -> T {
		match self {
			RpcEncodingConfigWrapper::Deprecated(encoding) => T::new_with_encoding(encoding),
			RpcEncodingConfigWrapper::Current(config) => config.unwrap_or_default(),
		}
	}

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

pub trait EncodingConfig {
	fn new_with_encoding(encoding: &Option<UiTransactionEncoding>) -> Self;
}

#[skip_serializing_none]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockConfig {
	pub encoding: Option<UiTransactionEncoding>,
	pub transaction_details: Option<TransactionDetails>,
	pub rewards: Option<bool>,
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub max_supported_transaction_version: Option<u8>,
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
	pub fn rewards_only() -> Self {
		Self {
			transaction_details: Some(TransactionDetails::None),
			..Self::default()
		}
	}

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

#[skip_serializing_none]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionConfig {
	pub encoding: Option<UiTransactionEncoding>,
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub max_supported_transaction_version: Option<u8>,
}

impl EncodingConfig for RpcTransactionConfig {
	fn new_with_encoding(encoding: &Option<UiTransactionEncoding>) -> Self {
		Self {
			encoding: *encoding,
			..Self::default()
		}
	}
}

#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcBlocksConfigWrapper {
	EndSlotOnly(Option<Slot>),
	CommitmentOnly(Option<CommitmentConfig>),
}

impl RpcBlocksConfigWrapper {
	pub fn unzip(&self) -> (Option<Slot>, Option<CommitmentConfig>) {
		match &self {
			RpcBlocksConfigWrapper::EndSlotOnly(end_slot) => (*end_slot, None),
			RpcBlocksConfigWrapper::CommitmentOnly(commitment) => (None, *commitment),
		}
	}
}

#[skip_serializing_none]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcContextConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub min_context_slot: Option<Slot>,
}

#[derive(Debug, Default)]
pub struct GetConfirmedSignaturesForAddress2Config {
	pub before: Option<Signature>,
	pub until: Option<Signature>,
	pub limit: Option<usize>,
	pub commitment: Option<CommitmentConfig>,
}

#[skip_serializing_none]
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcTransactionLogsConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcTransactionLogsFilter {
	All,
	AllWithVotes,
	Mentions(Vec<String>), // base58-encoded list of addresses
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcAccountSubscribeConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub encoding: Option<UiTransactionEncoding>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[builder(field_defaults(default, setter(strip_option)))]
#[serde(rename_all = "camelCase")]
pub struct RpcBlockSubscribeConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub encoding: Option<UiTransactionEncoding>,
	pub transaction_details: Option<TransactionDetails>,
	pub show_rewards: Option<bool>,
	pub max_supported_transaction_version: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpcBlockSubscribeFilter {
	All,
	MentionsAccountOrProgram(String),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TypedBuilder)]
#[serde(rename_all = "camelCase")]
#[builder(field_defaults(default, setter(strip_option)))]
pub struct RpcSignatureSubscribeConfig {
	#[serde(flatten)]
	pub commitment: Option<CommitmentConfig>,
	pub enable_received_notification: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct BlockSubscribeRequest {
	pub filter: RpcBlockSubscribeFilter,
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

#[derive(Debug, Clone, PartialEq, Eq, TypedBuilder)]
pub struct LogsSubscribeRequest {
	pub filter: RpcTransactionLogsFilter,
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
