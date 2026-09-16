use std::collections::HashMap;

use inflector::Inflector;
use serde::Deserialize;
use serde::Serialize;
pub use solana_account_decoder_client_types_wasm::ParsedAccount;
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

use crate::parse_address_lookup_table::parse_address_lookup_table;
use crate::parse_bpf_loader::parse_bpf_upgradeable_loader;
use crate::parse_config::parse_config;
use crate::parse_nonce::parse_nonce;
use crate::parse_stake::parse_stake;
use crate::parse_sysvar::parse_sysvar;
use crate::parse_token::parse_token_v3;
use crate::parse_vote::parse_vote;

/// Map of program ids that [`parse_account_data_v3`] knows how to parse to the
/// [`ParsableAccount`] variant describing them.
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
		m.insert(
			spl_token_2022_interface::id(),
			ParsableAccount::SplToken2022,
		);
		m.insert(spl_token_interface::id(), ParsableAccount::SplToken);
		m.insert(stake::id(), ParsableAccount::Stake);
		m.insert(sysvar::id(), ParsableAccount::Sysvar);
		m.insert(vote::id(), ParsableAccount::Vote);
		m
	});

/// Errors returned while parsing account data.
#[derive(Error, Debug)]
pub enum ParseAccountError {
	/// The account's program id is known but the data could not be decoded.
	#[error("{0:?} account not parsable")]
	AccountNotParsable(ParsableAccount),

	/// The account's program id is not one of the known parsable programs.
	#[error("Program not parsable")]
	ProgramNotParsable,

	/// The parser requires additional data (such as mint decimals) that was not
	/// supplied.
	#[error("Additional data required to parse: {0}")]
	AdditionalDataMissing(String),

	/// An instruction error was encountered while parsing.
	#[error("Instruction error")]
	InstructionError(#[from] InstructionError),

	/// Serializing the parsed account to JSON failed.
	#[error("Serde json error")]
	SerdeJsonError(#[from] serde_json::error::Error),
}

/// Programs whose account data can be decoded into a parsed JSON form.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParsableAccount {
	/// The address lookup table program.
	AddressLookupTable,
	/// The upgradeable BPF loader program.
	BpfUpgradeableLoader,
	/// The deprecated config program.
	Config,
	/// The system program (nonce accounts).
	Nonce,
	/// The SPL Token program.
	SplToken,
	/// The SPL Token-2022 program.
	SplToken2022,
	/// The stake program.
	Stake,
	/// Any sysvar account, dispatched by its specific address.
	Sysvar,
	/// The vote program.
	Vote,
}

/// Optional extra data needed to fully parse token accounts.
#[derive(Clone, Copy, Default)]
pub struct AccountAdditionalDataV3 {
	/// Additional data for SPL Token accounts, if the mint metadata is known.
	pub spl_token_additional_data: Option<SplTokenAdditionalDataV2>,
}

/// Extra token mint data needed to parse token accounts.
#[derive(Clone, Copy, Default)]
pub struct SplTokenAdditionalData {
	/// Number of decimals of the token mint.
	pub decimals: u8,
	/// The mint's interest bearing config and the unix timestamp used to
	/// calculate the current interest.
	pub interest_bearing_config: Option<(InterestBearingConfig, UnixTimestamp)>,
}

impl SplTokenAdditionalData {
	/// Create additional data with the given number of decimals.
	pub fn with_decimals(decimals: u8) -> Self {
		Self {
			decimals,
			..Default::default()
		}
	}
}

/// Extra token mint data needed to parse token accounts, including Token-2022
/// extensions.
#[derive(Clone, Copy, Default)]
pub struct SplTokenAdditionalDataV2 {
	/// Number of decimals of the token mint.
	pub decimals: u8,
	/// The mint's interest bearing config and the unix timestamp used to
	/// calculate the current interest.
	pub interest_bearing_config: Option<(InterestBearingConfig, UnixTimestamp)>,
	/// The mint's scaled UI amount config and the unix timestamp used to
	/// calculate the current multiplier.
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
	/// Create additional data with the given number of decimals.
	pub fn with_decimals(decimals: u8) -> Self {
		Self {
			decimals,
			..Default::default()
		}
	}
}

/// Parse an account's data into the JSON form served by
/// `getAccountInfo` with `jsonParsed` encoding.
///
/// `program_id` selects the parser through [`PARSABLE_PROGRAM_IDS`], and
/// `additional_data` supplies mint metadata for token accounts. Returns a
/// [`ParsedAccount`] whose `program` field is the kebab-cased
/// [`ParsableAccount`] name.
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
		ParsableAccount::Vote => serde_json::to_value(parse_vote(data, pubkey)?)?,
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
	use solana_vote_interface::state::VoteStateV4;
	use solana_vote_interface::state::VoteStateVersions;

	use super::*;

	#[test]
	fn test_parse_account_data() {
		let account_pubkey = solana_pubkey::new_rand();
		let other_program = solana_pubkey::new_rand();
		let data = vec![0; 4];
		assert!(parse_account_data_v3(&account_pubkey, &other_program, &data, None).is_err());

		let vote_state = VoteStateV4::default();
		let mut vote_account_data: Vec<u8> = vec![0; VoteStateV4::size_of()];
		let versioned = VoteStateVersions::new_v4(vote_state);
		VoteStateV4::serialize(&versioned, &mut vote_account_data).unwrap();
		let parsed = parse_account_data_v3(
			&account_pubkey,
			&vote_program_id(),
			&vote_account_data,
			None,
		)
		.unwrap();
		assert_eq!(parsed.program, "vote".to_string());
		assert_eq!(parsed.space, VoteStateV4::size_of() as u64);

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
