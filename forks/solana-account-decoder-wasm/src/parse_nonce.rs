use serde::Deserialize;
use serde::Serialize;
use solana_instruction::error::InstructionError;
use solana_nonce::state::State;
use solana_nonce::versions::Versions;

use crate::UiFeeCalculator;
use crate::parse_account_data::ParseAccountError;

/// Deserialize a nonce account into its JSON form.
///
/// Returns [`InstructionError::InvalidAccountData`] for uninitialized or
/// otherwise malformed accounts, so that empty system accounts are never
/// reported as uninitialized nonces.
pub fn parse_nonce(data: &[u8]) -> Result<UiNonceState, ParseAccountError> {
	let nonce_versions: Versions = bincode::deserialize(data)
		.map_err(|_| ParseAccountError::from(InstructionError::InvalidAccountData))?;
	match nonce_versions.state() {
		// This prevents parsing an allocated System-owned account with empty data of any non-zero
		// length as `uninitialized` nonce. An empty account of the wrong length can never be
		// initialized as a nonce account, and an empty account of the correct length may not be an
		// uninitialized nonce account, since it can be assigned to another program.
		State::Uninitialized => {
			Err(ParseAccountError::from(
				InstructionError::InvalidAccountData,
			))
		}
		State::Initialized(data) => {
			Ok(UiNonceState::Initialized(UiNonceData {
				authority: data.authority.to_string(),
				blockhash: data.blockhash().to_string(),
				fee_calculator: data.fee_calculator.into(),
			}))
		}
	}
}

/// A duplicate representation of `NonceState` for pretty JSON serialization
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum UiNonceState {
	/// The account is not an initialized nonce.
	Uninitialized,
	/// The account holds an initialized nonce.
	Initialized(UiNonceData),
}

/// Parsed nonce account data.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiNonceData {
	/// The authority allowed to advance the nonce, as a string.
	pub authority: String,
	/// The stored durable nonce, as a string.
	pub blockhash: String,
	/// The fee schedule in effect when the nonce was created.
	pub fee_calculator: UiFeeCalculator,
}

#[cfg(test)]
mod test {
	use solana_hash::Hash;
	use solana_nonce::state::Data;
	use solana_nonce::state::State;
	use solana_nonce::versions::Versions;
	use solana_pubkey::Pubkey;

	use super::*;

	#[test]
	fn test_parse_nonce() {
		let nonce_data = Versions::new(State::Initialized(Data::default()));
		let nonce_account_data = bincode::serialize(&nonce_data).unwrap();
		assert_eq!(
			parse_nonce(&nonce_account_data).unwrap(),
			UiNonceState::Initialized(UiNonceData {
				authority: Pubkey::default().to_string(),
				blockhash: Hash::default().to_string(),
				fee_calculator: UiFeeCalculator {
					lamports_per_signature: 0.to_string(),
				},
			}),
		);

		let bad_data = vec![0; 4];
		assert!(parse_nonce(&bad_data).is_err());
	}
}
