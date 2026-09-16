use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use bincode::deserialize;
use bincode::serialized_size;
use serde::Deserialize;
use serde::Serialize;
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_pubkey::Pubkey;

use crate::UiAccountData;
use crate::UiAccountEncoding;
use crate::parse_account_data::ParsableAccount;
use crate::parse_account_data::ParseAccountError;

/// Deserialize an upgradeable loader account into its JSON form, base64
/// encoding the program bytes of buffer and program data accounts.
///
/// Returns [`ParseAccountError::AccountNotParsable`] if the data is not a valid
/// bincode serialized [`UpgradeableLoaderState`].
pub fn parse_bpf_upgradeable_loader(
	data: &[u8],
) -> Result<BpfUpgradeableLoaderAccountType, ParseAccountError> {
	let account_state: UpgradeableLoaderState = deserialize(data).map_err(|_| {
		ParseAccountError::AccountNotParsable(ParsableAccount::BpfUpgradeableLoader)
	})?;
	let parsed_account = match account_state {
		UpgradeableLoaderState::Uninitialized => BpfUpgradeableLoaderAccountType::Uninitialized,
		UpgradeableLoaderState::Buffer { authority_address } => {
			let offset = if authority_address.is_some() {
				UpgradeableLoaderState::size_of_buffer_metadata()
			} else {
				// This case included for code completeness; in practice, a
				// Buffer account will always have
				// authority_address.is_some()
				UpgradeableLoaderState::size_of_buffer_metadata()
					- serialized_size(&Pubkey::default()).unwrap() as usize
			};
			BpfUpgradeableLoaderAccountType::Buffer(UiBuffer {
				authority: authority_address.map(|pubkey| pubkey.to_string()),
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&data[offset..]),
					UiAccountEncoding::Base64,
				),
			})
		}
		UpgradeableLoaderState::Program {
			programdata_address,
		} => {
			BpfUpgradeableLoaderAccountType::Program(UiProgram {
				program_data: programdata_address.to_string(),
			})
		}
		UpgradeableLoaderState::ProgramData {
			slot,
			upgrade_authority_address,
		} => {
			let offset = if upgrade_authority_address.is_some() {
				UpgradeableLoaderState::size_of_programdata_metadata()
			} else {
				UpgradeableLoaderState::size_of_programdata_metadata()
					- serialized_size(&Pubkey::default()).unwrap() as usize
			};
			BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData {
				slot,
				authority: upgrade_authority_address.map(|pubkey| pubkey.to_string()),
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&data[offset..]),
					UiAccountEncoding::Base64,
				),
			})
		}
	};
	Ok(parsed_account)
}

/// The parsed contents of an upgradeable loader account, tagged by state in
/// JSON.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum BpfUpgradeableLoaderAccountType {
	/// The account has not been initialized.
	Uninitialized,
	/// The account is a program buffer holding a pending deployment.
	Buffer(UiBuffer),
	/// The account is an executable program.
	Program(UiProgram),
	/// The account holds the deployed program data and upgrade authority.
	ProgramData(UiProgramData),
}

/// Parsed buffer account holding program bytes that are not yet deployed.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiBuffer {
	/// The authority allowed to write to the buffer, if set.
	pub authority: Option<String>,
	/// The buffered program data, base64 encoded.
	pub data: UiAccountData,
}

/// Parsed executable program account.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiProgram {
	/// The address of the program data account holding the deployed bytes.
	pub program_data: String,
}

/// Parsed program data account holding the deployed program bytes.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiProgramData {
	/// The slot at which the program was last deployed or upgraded.
	pub slot: u64,
	/// The upgrade authority, if set. Absent means the program is immutable.
	pub authority: Option<String>,
	/// The deployed program data, base64 encoded.
	pub data: UiAccountData,
}

#[cfg(test)]
mod test {
	use bincode::serialize;
	use solana_pubkey::Pubkey;

	use super::*;

	#[test]
	fn test_parse_bpf_upgradeable_loader_accounts() {
		let bpf_loader_state = UpgradeableLoaderState::Uninitialized;
		let account_data = serialize(&bpf_loader_state).unwrap();
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::Uninitialized
		);

		let program = vec![7u8; 64]; // Arbitrary program data

		let authority = Pubkey::new_unique();
		let bpf_loader_state = UpgradeableLoaderState::Buffer {
			authority_address: Some(authority),
		};
		let mut account_data = serialize(&bpf_loader_state).unwrap();
		account_data.extend_from_slice(&program);
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::Buffer(UiBuffer {
				authority: Some(authority.to_string()),
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&program),
					UiAccountEncoding::Base64
				),
			})
		);

		// This case included for code completeness; in practice, a Buffer
		// account will always have authority_address.is_some()
		let bpf_loader_state = UpgradeableLoaderState::Buffer {
			authority_address: None,
		};
		let mut account_data = serialize(&bpf_loader_state).unwrap();
		account_data.extend_from_slice(&program);
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::Buffer(UiBuffer {
				authority: None,
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&program),
					UiAccountEncoding::Base64
				),
			})
		);

		let programdata_address = Pubkey::new_unique();
		let bpf_loader_state = UpgradeableLoaderState::Program {
			programdata_address,
		};
		let account_data = serialize(&bpf_loader_state).unwrap();
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::Program(UiProgram {
				program_data: programdata_address.to_string(),
			})
		);

		let authority = Pubkey::new_unique();
		let slot = 42;
		let bpf_loader_state = UpgradeableLoaderState::ProgramData {
			slot,
			upgrade_authority_address: Some(authority),
		};
		let mut account_data = serialize(&bpf_loader_state).unwrap();
		account_data.extend_from_slice(&program);
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData {
				slot,
				authority: Some(authority.to_string()),
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&program),
					UiAccountEncoding::Base64
				),
			})
		);

		let bpf_loader_state = UpgradeableLoaderState::ProgramData {
			slot,
			upgrade_authority_address: None,
		};
		let mut account_data = serialize(&bpf_loader_state).unwrap();
		account_data.extend_from_slice(&program);
		assert_eq!(
			parse_bpf_upgradeable_loader(&account_data).unwrap(),
			BpfUpgradeableLoaderAccountType::ProgramData(UiProgramData {
				slot,
				authority: None,
				data: UiAccountData::Binary(
					BASE64_STANDARD.encode(&program),
					UiAccountEncoding::Base64
				),
			})
		);
	}
}
