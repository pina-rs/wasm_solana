use serde::Deserialize;
use serde::Serialize;
/// Maximum length of a validator info key.
pub const MAX_SHORT_FIELD_LENGTH: usize = 80;
/// Maximum length of a validator info value.
pub const MAX_LONG_FIELD_LENGTH: usize = 300;
/// Maximum size of validator configuration data (`ValidatorInfo`).
pub const MAX_VALIDATOR_INFO: u64 = 576;

solana_pubkey::declare_id!("Va1idator1nfo111111111111111111111111111111");

/// Validator metadata stored in the validator info config account.
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize, Default)]
pub struct ValidatorInfo {
	/// The JSON encoded validator info payload.
	pub info: String,
}
