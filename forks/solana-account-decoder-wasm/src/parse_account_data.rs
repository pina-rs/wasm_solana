use std::collections::HashMap;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use inflector::Inflector;
use serde::Deserialize;
use serde::Serialize;
use solana_account::ReadableAccount;
pub use solana_account_decoder_client_types_wasm::ParsedAccount;
use solana_account_decoder_client_types_wasm::UiAccount;
use solana_account_decoder_client_types_wasm::UiAccountData;
use solana_account_decoder_client_types_wasm::UiAccountEncoding;
use solana_account_decoder_client_types_wasm::UiDataSliceConfig;
use solana_clock::UnixTimestamp;
use solana_instruction::error::InstructionError;
use solana_pubkey::Pubkey;
use solana_sdk_ids::address_lookup_table;
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_sdk_ids::config;
use solana_sdk_ids::stake;
use solana_sdk_ids::system_program;
use solana_sdk_ids::sysvar;
use solana_sdk_ids::vote;
use spl_token_2022_interface::extension::interest_bearing_mint::InterestBearingConfig;
use spl_token_2022_interface::extension::scaled_ui_amount::ScaledUiAmountConfig;
use thiserror::Error;

use crate::MAX_BASE58_BYTES;
use crate::parse_address_lookup_table::parse_address_lookup_table;
use crate::parse_bpf_loader::parse_bpf_upgradeable_loader;
use crate::parse_config::parse_config;
use crate::parse_nonce::parse_nonce;
use crate::parse_stake::parse_stake;
use crate::parse_sysvar::parse_sysvar;
use crate::parse_token::parse_token_v3;
use crate::parse_vote::parse_vote;
use crate::slice_data;

pub static PARSABLE_PROGRAM_IDS: std::sync::LazyLock<HashMap<Pubkey, ParsableAccount>> =
	std::sync::LazyLock::new(|| {
		let mut m = HashMap::new();
		m.insert(
			address_lookup_table::id(),
			ParsableAccount::AddressLookupTable,
		);
		m.insert(
			bpf_loader_upgradeable::id(),
			ParsableAccount::BpfUpgradeableLoader,
		);
		m.insert(config::id(), ParsableAccount::Config);
		m.insert(system_program::id(), ParsableAccount::Nonce);
		m.insert(spl_token_interface::id(), ParsableAccount::SplToken);
		m.insert(
			spl_token_2022_interface::id(),
			ParsableAccount::SplToken2022,
		);
		m.insert(stake::id(), ParsableAccount::Stake);
		m.insert(sysvar::id(), ParsableAccount::Sysvar);
		m.insert(vote::id(), ParsableAccount::Vote);
		m
	});

#[derive(Error, Debug)]
pub enum ParseAccountError {
	#[error("{0:?} account not parsable")]
	AccountNotParsable(ParsableAccount),

	#[error("Program not parsable")]
	ProgramNotParsable,

	#[error("Additional data required to parse: {0}")]
	AdditionalDataMissing(String),

	#[error("Instruction error")]
	InstructionError(#[from] InstructionError),

	#[error("Serde json error")]
	SerdeJsonError(#[from] serde_json::error::Error),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParsableAccount {
	AddressLookupTable,
	BpfUpgradeableLoader,
	Config,
	Nonce,
	SplToken,
	SplToken2022,
	Stake,
	Sysvar,
	Vote,
}

pub trait EncodeUiAccount {}

fn encode_ui_accout_bs58<T: ReadableAccount>(
	account: &T,
	data_slice_config: Option<UiDataSliceConfig>,
) -> String {
	let slice = slice_data(account.data(), data_slice_config);
	if slice.len() <= MAX_BASE58_BYTES {
		bs58::encode(slice).into_string()
	} else {
		"error: data too large for bs58 encoding".to_string()
	}
}

pub fn encode_ui_account<T: ReadableAccount>(
	pubkey: &Pubkey,
	account: &T,
	encoding: UiAccountEncoding,
	additional_data: Option<AccountAdditionalDataV3>,
	data_slice_config: Option<UiDataSliceConfig>,
) -> UiAccount {
	let space = account.data().len();
	let data = match encoding {
		UiAccountEncoding::Binary => {
			let data = encode_ui_accout_bs58(account, data_slice_config);
			UiAccountData::LegacyBinary(data)
		}
		UiAccountEncoding::Base58 => {
			let data = encode_ui_accout_bs58(account, data_slice_config);
			UiAccountData::Binary(data, encoding)
		}
		UiAccountEncoding::Base64 => {
			UiAccountData::Binary(
				BASE64_STANDARD.encode(slice_data(account.data(), data_slice_config)),
				encoding,
			)
		}
		#[cfg(not(feature = "zstd"))]
		UiAccountEncoding::Base64Zstd => todo!("zstd not supported without the zstd feature flag"),
		#[cfg(feature = "zstd")]
		UiAccountEncoding::Base64Zstd => {
			use std::io::Write;

			let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 0).unwrap();
			match encoder
				.write_all(slice_data(account.data(), data_slice_config))
				.and_then(|()| encoder.finish())
			{
				Ok(zstd_data) => UiAccountData::Binary(BASE64_STANDARD.encode(zstd_data), encoding),
				Err(_) => {
					UiAccountData::Binary(
						BASE64_STANDARD.encode(slice_data(account.data(), data_slice_config)),
						UiAccountEncoding::Base64,
					)
				}
			}
		}
		UiAccountEncoding::JsonParsed => {
			if let Ok(parsed_data) =
				parse_account_data_v3(pubkey, account.owner(), account.data(), additional_data)
			{
				UiAccountData::Json(parsed_data)
			} else {
				UiAccountData::Binary(
					BASE64_STANDARD.encode(slice_data(account.data(), data_slice_config)),
					UiAccountEncoding::Base64,
				)
			}
		}
	};
	UiAccount {
		lamports: account.lamports(),
		data,
		owner: *account.owner(),
		executable: account.executable(),
		rent_epoch: account.rent_epoch(),
		space: Some(space as u64),
	}
}

#[derive(Clone, Copy, Default)]
pub struct AccountAdditionalDataV3 {
	pub spl_token_additional_data: Option<SplTokenAdditionalDataV2>,
}

#[derive(Clone, Copy, Default)]
pub struct SplTokenAdditionalData {
	pub decimals: u8,
	pub interest_bearing_config: Option<(InterestBearingConfig, UnixTimestamp)>,
}

impl SplTokenAdditionalData {
	pub fn with_decimals(decimals: u8) -> Self {
		Self {
			decimals,
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Default)]
pub struct SplTokenAdditionalDataV2 {
	pub decimals: u8,
	pub interest_bearing_config: Option<(InterestBearingConfig, UnixTimestamp)>,
	pub scaled_ui_amount_config: Option<(ScaledUiAmountConfig, UnixTimestamp)>,
}

impl From<SplTokenAdditionalData> for SplTokenAdditionalDataV2 {
	fn from(v: SplTokenAdditionalData) -> Self {
		Self {
			decimals: v.decimals,
			interest_bearing_config: v.interest_bearing_config,
			scaled_ui_amount_config: None,
		}
	}
}

impl SplTokenAdditionalDataV2 {
	pub fn with_decimals(decimals: u8) -> Self {
		Self {
			decimals,
			..Default::default()
		}
	}
}

pub fn parse_account_data_v3(
	pubkey: &Pubkey,
	program_id: &Pubkey,
	data: &[u8],
	additional_data: Option<AccountAdditionalDataV3>,
) -> Result<ParsedAccount, ParseAccountError> {
	let program_name = PARSABLE_PROGRAM_IDS
		.get(program_id)
		.ok_or(ParseAccountError::ProgramNotParsable)?;
	let additional_data = additional_data.unwrap_or_default();
	let parsed_json = match program_name {
		ParsableAccount::AddressLookupTable => {
			serde_json::to_value(parse_address_lookup_table(data)?)?
		}
		ParsableAccount::BpfUpgradeableLoader => {
			serde_json::to_value(parse_bpf_upgradeable_loader(data)?)?
		}
		ParsableAccount::Config => serde_json::to_value(parse_config(data, pubkey)?)?,
		ParsableAccount::Nonce => serde_json::to_value(parse_nonce(data)?)?,
		ParsableAccount::SplToken | ParsableAccount::SplToken2022 => {
			serde_json::to_value(parse_token_v3(
				data,
				additional_data.spl_token_additional_data.as_ref(),
			)?)?
		}
		ParsableAccount::Stake => serde_json::to_value(parse_stake(data)?)?,
		ParsableAccount::Sysvar => serde_json::to_value(parse_sysvar(data, pubkey)?)?,
		ParsableAccount::Vote => serde_json::to_value(parse_vote(data)?)?,
	};
	Ok(ParsedAccount {
		program: format!("{program_name:?}").to_kebab_case(),
		parsed: parsed_json,
		space: data.len() as u64,
	})
}

#[cfg(test)]
mod test {
	use solana_nonce::state::Data;
	use solana_nonce::state::State;
	use solana_nonce::versions::Versions;
	use solana_vote_interface::program::id as vote_program_id;
	use solana_vote_interface::state::VoteStateV3;
	use solana_vote_interface::state::VoteStateVersions;

	use super::*;

	#[test]
	fn test_parse_account_data() {
		let account_pubkey = solana_pubkey::new_rand();
		let other_program = solana_pubkey::new_rand();
		let data = vec![0; 4];
		assert!(parse_account_data_v3(&account_pubkey, &other_program, &data, None).is_err());

		let vote_state = VoteStateV3::default();
		let mut vote_account_data: Vec<u8> = vec![0; VoteStateV3::size_of()];
		let versioned = VoteStateVersions::new_v3(vote_state);
		VoteStateV3::serialize(&versioned, &mut vote_account_data).unwrap();
		let parsed = parse_account_data_v3(
			&account_pubkey,
			&vote_program_id(),
			&vote_account_data,
			None,
		)
		.unwrap();
		assert_eq!(parsed.program, "vote".to_string());
		assert_eq!(parsed.space, VoteStateV3::size_of() as u64);

		let nonce_data = Versions::new(State::Initialized(Data::default()));
		let nonce_account_data = bincode::serialize(&nonce_data).unwrap();
		let parsed = parse_account_data_v3(
			&account_pubkey,
			&system_program::id(),
			&nonce_account_data,
			None,
		)
		.unwrap();
		assert_eq!(parsed.program, "nonce".to_string());
		assert_eq!(parsed.space, State::size() as u64);
	}
}
