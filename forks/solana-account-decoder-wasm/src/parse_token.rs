use std::str::FromStr;

pub use solana_account_decoder_client_types_wasm::token::TokenAccountType;
pub use solana_account_decoder_client_types_wasm::token::UiAccountState;
pub use solana_account_decoder_client_types_wasm::token::UiMint;
pub use solana_account_decoder_client_types_wasm::token::UiMultisig;
pub use solana_account_decoder_client_types_wasm::token::UiTokenAccount;
pub use solana_account_decoder_client_types_wasm::token::UiTokenAmount;
pub use solana_account_decoder_client_types_wasm::token::real_number_string;
pub use solana_account_decoder_client_types_wasm::token::real_number_string_trimmed;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
pub use spl_generic_token::is_known_spl_token_id;
pub use spl_generic_token::spl_token_ids;
use spl_token_2022_interface::extension::BaseStateWithExtensions;
use spl_token_2022_interface::extension::StateWithExtensions;
use spl_token_2022_interface::generic_token_account::GenericTokenAccount;
use spl_token_2022_interface::state::Account;
use spl_token_2022_interface::state::AccountState;
use spl_token_2022_interface::state::Mint;
use spl_token_2022_interface::state::Multisig;

use crate::parse_account_data::ParsableAccount;
use crate::parse_account_data::ParseAccountError;
use crate::parse_account_data::SplTokenAdditionalDataV2;
use crate::parse_token_extension::parse_extension;

pub fn parse_token_v3(
	data: &[u8],
	additional_data: Option<&SplTokenAdditionalDataV2>,
) -> Result<TokenAccountType, ParseAccountError> {
	if let Ok(account) = StateWithExtensions::<Account>::unpack(data) {
		let additional_data = additional_data.as_ref().ok_or_else(|| {
			ParseAccountError::AdditionalDataMissing(
				"no mint_decimals provided to parse spl-token account".to_string(),
			)
		})?;
		let extension_types = account.get_extension_types().unwrap_or_default();
		let ui_extensions = extension_types
			.iter()
			.map(|extension_type| parse_extension::<Account>(extension_type, &account))
			.collect();
		return Ok(TokenAccountType::Account(UiTokenAccount {
			mint: account.base.mint.to_string(),
			owner: account.base.owner.to_string(),
			token_amount: token_amount_to_ui_amount_v3(account.base.amount, additional_data),
			delegate: match account.base.delegate {
				COption::Some(pubkey) => Some(pubkey.to_string()),
				COption::None => None,
			},
			state: convert_account_state(account.base.state),
			is_native: account.base.is_native(),
			rent_exempt_reserve: match account.base.is_native {
				COption::Some(reserve) => {
					Some(token_amount_to_ui_amount_v3(reserve, additional_data))
				}
				COption::None => None,
			},
			delegated_amount: if account.base.delegate.is_none() {
				None
			} else {
				Some(token_amount_to_ui_amount_v3(
					account.base.delegated_amount,
					additional_data,
				))
			},
			close_authority: match account.base.close_authority {
				COption::Some(pubkey) => Some(pubkey.to_string()),
				COption::None => None,
			},
			extensions: ui_extensions,
		}));
	}
	if let Ok(mint) = StateWithExtensions::<Mint>::unpack(data) {
		let extension_types = mint.get_extension_types().unwrap_or_default();
		let ui_extensions = extension_types
			.iter()
			.map(|extension_type| parse_extension::<Mint>(extension_type, &mint))
			.collect();
		return Ok(TokenAccountType::Mint(UiMint {
			mint_authority: match mint.base.mint_authority {
				COption::Some(pubkey) => Some(pubkey.to_string()),
				COption::None => None,
			},
			supply: mint.base.supply.to_string(),
			decimals: mint.base.decimals,
			is_initialized: mint.base.is_initialized,
			freeze_authority: match mint.base.freeze_authority {
				COption::Some(pubkey) => Some(pubkey.to_string()),
				COption::None => None,
			},
			extensions: ui_extensions,
		}));
	}
	if data.len() == Multisig::get_packed_len() {
		let multisig = Multisig::unpack(data)
			.map_err(|_| ParseAccountError::AccountNotParsable(ParsableAccount::SplToken))?;
		Ok(TokenAccountType::Multisig(UiMultisig {
			num_required_signers: multisig.m,
			num_valid_signers: multisig.n,
			is_initialized: multisig.is_initialized,
			signers: multisig
				.signers
				.iter()
				.filter_map(|pubkey| {
					if pubkey == &Pubkey::default() {
						None
					} else {
						Some(pubkey.to_string())
					}
				})
				.collect(),
		}))
	} else {
		Err(ParseAccountError::AccountNotParsable(
			ParsableAccount::SplToken,
		))
	}
}

pub fn convert_account_state(state: AccountState) -> UiAccountState {
	match state {
		AccountState::Uninitialized => UiAccountState::Uninitialized,
		AccountState::Initialized => UiAccountState::Initialized,
		AccountState::Frozen => UiAccountState::Frozen,
	}
}

pub fn token_amount_to_ui_amount_v3(
	amount: u64,
	additional_data: &SplTokenAdditionalDataV2,
) -> UiTokenAmount {
	let decimals = additional_data.decimals;
	let (ui_amount, ui_amount_string) = if let Some((interest_bearing_config, unix_timestamp)) =
		additional_data.interest_bearing_config
	{
		let ui_amount_string =
			interest_bearing_config.amount_to_ui_amount(amount, decimals, unix_timestamp);
		(
			ui_amount_string
				.as_ref()
				.and_then(|x| f64::from_str(x).ok()),
			ui_amount_string.unwrap_or(String::new()),
		)
	} else if let Some((scaled_ui_amount_config, unix_timestamp)) =
		additional_data.scaled_ui_amount_config
	{
		let ui_amount_string =
			scaled_ui_amount_config.amount_to_ui_amount(amount, decimals, unix_timestamp);
		(
			ui_amount_string
				.as_ref()
				.and_then(|x| f64::from_str(x).ok()),
			ui_amount_string.unwrap_or(String::new()),
		)
	} else {
		let ui_amount = 10_usize
			.checked_pow(u32::from(decimals))
			.map(|dividend| amount as f64 / dividend as f64);
		(ui_amount, real_number_string_trimmed(amount, decimals))
	};
	UiTokenAmount {
		ui_amount,
		decimals,
		amount: amount.to_string(),
		ui_amount_string,
	}
}

pub fn get_token_account_mint(data: &[u8]) -> Option<Pubkey> {
	Account::valid_account_data(data)
		.then(|| Pubkey::try_from(data.get(..32)?).ok())
		.flatten()
}

#[cfg(test)]
mod test {
	use solana_account_decoder_client_types_wasm::token::UiExtension;
	use spl_token_2022_interface::extension::BaseStateWithExtensionsMut;
	use spl_token_2022_interface::extension::ExtensionType;
	use spl_token_2022_interface::extension::StateWithExtensionsMut;
	use spl_token_2022_interface::extension::immutable_owner::ImmutableOwner;
	use spl_token_2022_interface::extension::interest_bearing_mint::InterestBearingConfig;
	use spl_token_2022_interface::extension::memo_transfer::MemoTransfer;
	use spl_token_2022_interface::extension::mint_close_authority::MintCloseAuthority;
	use spl_token_2022_interface::extension::scaled_ui_amount::ScaledUiAmountConfig;

	use super::*;
	use crate::parse_token_extension::UiMemoTransfer;
	use crate::parse_token_extension::UiMintCloseAuthority;

	const INT_SECONDS_PER_YEAR: i64 = 6 * 6 * 24 * 36524;

	#[test]
	fn test_parse_token() {
		let mint_pubkey = Pubkey::new_from_array([2; 32]);
		let owner_pubkey = Pubkey::new_from_array([3; 32]);
		let mut account_data = vec![0; Account::get_packed_len()];
		let mut account = Account::unpack_unchecked(&account_data).unwrap();
		account.mint = mint_pubkey;
		account.owner = owner_pubkey;
		account.amount = 42;
		account.state = AccountState::Initialized;
		account.is_native = COption::None;
		account.close_authority = COption::Some(owner_pubkey);
		Account::pack(account, &mut account_data).unwrap();

		assert!(parse_token_v3(&account_data, None).is_err());
		assert_eq!(
			parse_token_v3(
				&account_data,
				Some(&SplTokenAdditionalDataV2::with_decimals(2))
			)
			.unwrap(),
			TokenAccountType::Account(UiTokenAccount {
				mint: mint_pubkey.to_string(),
				owner: owner_pubkey.to_string(),
				token_amount: UiTokenAmount {
					ui_amount: Some(0.42),
					decimals: 2,
					amount: "42".to_string(),
					ui_amount_string: "0.42".to_string()
				},
				delegate: None,
				state: UiAccountState::Initialized,
				is_native: false,
				rent_exempt_reserve: None,
				delegated_amount: None,
				close_authority: Some(owner_pubkey.to_string()),
				extensions: vec![],
			}),
		);

		let mut mint_data = vec![0; Mint::get_packed_len()];
		let mut mint = Mint::unpack_unchecked(&mint_data).unwrap();
		mint.mint_authority = COption::Some(owner_pubkey);
		mint.supply = 42;
		mint.decimals = 3;
		mint.is_initialized = true;
		mint.freeze_authority = COption::Some(owner_pubkey);
		Mint::pack(mint, &mut mint_data).unwrap();

		assert_eq!(
			parse_token_v3(&mint_data, None).unwrap(),
			TokenAccountType::Mint(UiMint {
				mint_authority: Some(owner_pubkey.to_string()),
				supply: 42.to_string(),
				decimals: 3,
				is_initialized: true,
				freeze_authority: Some(owner_pubkey.to_string()),
				extensions: vec![],
			}),
		);

		let first_signer = Pubkey::new_from_array([1; 32]);
		let second_signer = Pubkey::new_from_array([2; 32]);
		let third_signer = Pubkey::new_from_array([3; 32]);
		let mut multisig_data = vec![0; Multisig::get_packed_len()];
		let mut signers = [Pubkey::default(); 11];
		signers[0] = first_signer;
		signers[1] = second_signer;
		signers[2] = third_signer;
		let mut multisig = Multisig::unpack_unchecked(&multisig_data).unwrap();
		multisig.m = 2;
		multisig.n = 3;
		multisig.is_initialized = true;
		multisig.signers = signers;
		Multisig::pack(multisig, &mut multisig_data).unwrap();

		assert_eq!(
			parse_token_v3(&multisig_data, None).unwrap(),
			TokenAccountType::Multisig(UiMultisig {
				num_required_signers: 2,
				num_valid_signers: 3,
				is_initialized: true,
				signers: vec![
					first_signer.to_string(),
					second_signer.to_string(),
					third_signer.to_string()
				],
			}),
		);

		let bad_data = vec![0; 4];
		assert!(parse_token_v3(&bad_data, None).is_err());
	}

	#[test]
	fn test_get_token_account_mint() {
		let mint_pubkey = Pubkey::new_from_array([2; 32]);
		let mut account_data = vec![0; Account::get_packed_len()];
		let mut account = Account::unpack_unchecked(&account_data).unwrap();
		account.mint = mint_pubkey;
		account.state = AccountState::Initialized;
		Account::pack(account, &mut account_data).unwrap();

		let expected_mint_pubkey = Pubkey::from([2; 32]);
		assert_eq!(
			get_token_account_mint(&account_data),
			Some(expected_mint_pubkey)
		);
	}

	#[test]
	fn test_ui_token_amount_real_string() {
		assert_eq!(&real_number_string(1, 0), "1");
		assert_eq!(&real_number_string_trimmed(1, 0), "1");
		let token_amount =
			token_amount_to_ui_amount_v3(1, &SplTokenAdditionalDataV2::with_decimals(0));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(1, 0)
		);
		assert_eq!(token_amount.ui_amount, Some(1.0));
		assert_eq!(&real_number_string(10, 0), "10");
		assert_eq!(&real_number_string_trimmed(10, 0), "10");
		let token_amount =
			token_amount_to_ui_amount_v3(10, &SplTokenAdditionalDataV2::with_decimals(0));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(10, 0)
		);
		assert_eq!(token_amount.ui_amount, Some(10.0));
		assert_eq!(&real_number_string(1, 9), "0.000000001");
		assert_eq!(&real_number_string_trimmed(1, 9), "0.000000001");
		let token_amount =
			token_amount_to_ui_amount_v3(1, &SplTokenAdditionalDataV2::with_decimals(9));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(1, 9)
		);
		assert_eq!(token_amount.ui_amount, Some(0.000_000_001));
		assert_eq!(&real_number_string(1_000_000_000, 9), "1.000000000");
		assert_eq!(&real_number_string_trimmed(1_000_000_000, 9), "1");
		let token_amount = token_amount_to_ui_amount_v3(
			1_000_000_000,
			&SplTokenAdditionalDataV2::with_decimals(9),
		);
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(1_000_000_000, 9)
		);
		assert_eq!(token_amount.ui_amount, Some(1.0));
		assert_eq!(&real_number_string(1_234_567_890, 3), "1234567.890");
		assert_eq!(&real_number_string_trimmed(1_234_567_890, 3), "1234567.89");
		let token_amount = token_amount_to_ui_amount_v3(
			1_234_567_890,
			&SplTokenAdditionalDataV2::with_decimals(3),
		);
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(1_234_567_890, 3)
		);
		assert_eq!(token_amount.ui_amount, Some(1_234_567.89));
		assert_eq!(
			&real_number_string(1_234_567_890, 25),
			"0.0000000000000001234567890"
		);
		assert_eq!(
			&real_number_string_trimmed(1_234_567_890, 25),
			"0.000000000000000123456789"
		);
		let token_amount = token_amount_to_ui_amount_v3(
			1_234_567_890,
			&SplTokenAdditionalDataV2::with_decimals(20),
		);
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(1_234_567_890, 20)
		);
		assert_eq!(token_amount.ui_amount, None);
	}

	#[test]
	fn test_ui_token_amount_with_interest() {
		// constant 5%
		let config = InterestBearingConfig {
			initialization_timestamp: 0.into(),
			pre_update_average_rate: 500.into(),
			last_update_timestamp: INT_SECONDS_PER_YEAR.into(),
			current_rate: 500.into(),
			..Default::default()
		};
		let additional_data = SplTokenAdditionalDataV2 {
			decimals: 18,
			interest_bearing_config: Some((config, INT_SECONDS_PER_YEAR)),
			..Default::default()
		};
		const ONE: u64 = 1_000_000_000_000_000_000;
		const TEN: u64 = 10_000_000_000_000_000_000;
		let token_amount = token_amount_to_ui_amount_v3(ONE, &additional_data);
		assert!(
			token_amount
				.ui_amount_string
				.starts_with("1.051271096376024117")
		);
		assert!(
			(token_amount.ui_amount.unwrap() - 1.051_271_096_376_024_1_f64).abs() < f64::EPSILON
		);
		let token_amount = token_amount_to_ui_amount_v3(TEN, &additional_data);
		assert!(
			token_amount
				.ui_amount_string
				.starts_with("10.512710963760241611")
		);
		assert!(
			(token_amount.ui_amount.unwrap() - 10.512_710_963_760_242_f64).abs() < f64::EPSILON
		);

		// huge case
		let config = InterestBearingConfig {
			initialization_timestamp: 0.into(),
			pre_update_average_rate: 32767.into(),
			last_update_timestamp: 0.into(),
			current_rate: 32767.into(),
			..Default::default()
		};
		let additional_data = SplTokenAdditionalDataV2 {
			decimals: 0,
			interest_bearing_config: Some((config, INT_SECONDS_PER_YEAR * 1_000)),
			..Default::default()
		};
		let token_amount = token_amount_to_ui_amount_v3(u64::MAX, &additional_data);
		assert_eq!(token_amount.ui_amount, Some(f64::INFINITY));
		assert_eq!(token_amount.ui_amount_string, "inf");
	}

	#[test]
	fn test_ui_token_amount_with_multiplier() {
		// 2x multiplier
		let config = ScaledUiAmountConfig {
			new_multiplier: 2f64.into(),
			..Default::default()
		};
		let additional_data = SplTokenAdditionalDataV2 {
			decimals: 18,
			scaled_ui_amount_config: Some((config, 0)),
			..Default::default()
		};
		const ONE: u64 = 1_000_000_000_000_000_000;
		const TEN: u64 = 10_000_000_000_000_000_000;
		let token_amount = token_amount_to_ui_amount_v3(ONE, &additional_data);
		assert_eq!(token_amount.ui_amount_string, "2");
		assert!(token_amount.ui_amount_string.starts_with('2'));
		assert!((token_amount.ui_amount.unwrap() - 2.0).abs() < f64::EPSILON);
		let token_amount = token_amount_to_ui_amount_v3(TEN, &additional_data);
		assert!(token_amount.ui_amount_string.starts_with("20"));
		assert!((token_amount.ui_amount.unwrap() - 20.0).abs() < f64::EPSILON);

		// huge case
		let config = ScaledUiAmountConfig {
			new_multiplier: f64::INFINITY.into(),
			..Default::default()
		};
		let additional_data = SplTokenAdditionalDataV2 {
			decimals: 0,
			scaled_ui_amount_config: Some((config, 0)),
			..Default::default()
		};
		let token_amount = token_amount_to_ui_amount_v3(u64::MAX, &additional_data);
		assert_eq!(token_amount.ui_amount, Some(f64::INFINITY));
		assert_eq!(token_amount.ui_amount_string, "inf");
	}

	#[test]
	fn test_ui_token_amount_real_string_zero() {
		assert_eq!(&real_number_string(0, 0), "0");
		assert_eq!(&real_number_string_trimmed(0, 0), "0");
		let token_amount =
			token_amount_to_ui_amount_v3(0, &SplTokenAdditionalDataV2::with_decimals(0));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(0, 0)
		);
		assert_eq!(token_amount.ui_amount, Some(0.0));
		assert_eq!(&real_number_string(0, 9), "0.000000000");
		assert_eq!(&real_number_string_trimmed(0, 9), "0");
		let token_amount =
			token_amount_to_ui_amount_v3(0, &SplTokenAdditionalDataV2::with_decimals(9));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(0, 9)
		);
		assert_eq!(token_amount.ui_amount, Some(0.0));
		assert_eq!(&real_number_string(0, 25), "0.0000000000000000000000000");
		assert_eq!(&real_number_string_trimmed(0, 25), "0");
		let token_amount =
			token_amount_to_ui_amount_v3(0, &SplTokenAdditionalDataV2::with_decimals(20));
		assert_eq!(
			token_amount.ui_amount_string,
			real_number_string_trimmed(0, 20)
		);
		assert_eq!(token_amount.ui_amount, None);
	}

	#[test]
	fn test_parse_token_account_with_extensions() {
		let mint_pubkey = Pubkey::new_from_array([2; 32]);
		let owner_pubkey = Pubkey::new_from_array([3; 32]);

		let account_base = Account {
			mint: mint_pubkey,
			owner: owner_pubkey,
			amount: 42,
			state: AccountState::Initialized,
			is_native: COption::None,
			close_authority: COption::Some(owner_pubkey),
			delegate: COption::None,
			delegated_amount: 0,
		};
		let account_size = ExtensionType::try_calculate_account_len::<Account>(&[
			ExtensionType::ImmutableOwner,
			ExtensionType::MemoTransfer,
		])
		.unwrap();
		let mut account_data = vec![0; account_size];
		let mut account_state =
			StateWithExtensionsMut::<Account>::unpack_uninitialized(&mut account_data).unwrap();

		account_state.base = account_base;
		account_state.pack_base();
		account_state.init_account_type().unwrap();

		assert!(parse_token_v3(&account_data, None).is_err());
		assert_eq!(
			parse_token_v3(
				&account_data,
				Some(&SplTokenAdditionalDataV2::with_decimals(2))
			)
			.unwrap(),
			TokenAccountType::Account(UiTokenAccount {
				mint: mint_pubkey.to_string(),
				owner: owner_pubkey.to_string(),
				token_amount: UiTokenAmount {
					ui_amount: Some(0.42),
					decimals: 2,
					amount: "42".to_string(),
					ui_amount_string: "0.42".to_string()
				},
				delegate: None,
				state: UiAccountState::Initialized,
				is_native: false,
				rent_exempt_reserve: None,
				delegated_amount: None,
				close_authority: Some(owner_pubkey.to_string()),
				extensions: vec![],
			}),
		);

		let mut account_data = vec![0; account_size];
		let mut account_state =
			StateWithExtensionsMut::<Account>::unpack_uninitialized(&mut account_data).unwrap();

		account_state.base = account_base;
		account_state.pack_base();
		account_state.init_account_type().unwrap();

		account_state
			.init_extension::<ImmutableOwner>(true)
			.unwrap();
		let memo_transfer = account_state.init_extension::<MemoTransfer>(true).unwrap();
		memo_transfer.require_incoming_transfer_memos = true.into();

		assert!(parse_token_v3(&account_data, None).is_err());
		assert_eq!(
			parse_token_v3(
				&account_data,
				Some(&SplTokenAdditionalDataV2::with_decimals(2))
			)
			.unwrap(),
			TokenAccountType::Account(UiTokenAccount {
				mint: mint_pubkey.to_string(),
				owner: owner_pubkey.to_string(),
				token_amount: UiTokenAmount {
					ui_amount: Some(0.42),
					decimals: 2,
					amount: "42".to_string(),
					ui_amount_string: "0.42".to_string()
				},
				delegate: None,
				state: UiAccountState::Initialized,
				is_native: false,
				rent_exempt_reserve: None,
				delegated_amount: None,
				close_authority: Some(owner_pubkey.to_string()),
				extensions: vec![
					UiExtension::ImmutableOwner,
					UiExtension::MemoTransfer(UiMemoTransfer {
						require_incoming_transfer_memos: true,
					}),
				],
			}),
		);
	}

	#[test]
	fn test_parse_token_mint_with_extensions() {
		let owner_pubkey = Pubkey::new_from_array([3; 32]);
		let mint_size =
			ExtensionType::try_calculate_account_len::<Mint>(&[ExtensionType::MintCloseAuthority])
				.unwrap();
		let mint_base = Mint {
			mint_authority: COption::Some(owner_pubkey),
			supply: 42,
			decimals: 3,
			is_initialized: true,
			freeze_authority: COption::Some(owner_pubkey),
		};
		let mut mint_data = vec![0; mint_size];
		let mut mint_state =
			StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();

		mint_state.base = mint_base;
		mint_state.pack_base();
		mint_state.init_account_type().unwrap();

		assert_eq!(
			parse_token_v3(&mint_data, None).unwrap(),
			TokenAccountType::Mint(UiMint {
				mint_authority: Some(owner_pubkey.to_string()),
				supply: 42.to_string(),
				decimals: 3,
				is_initialized: true,
				freeze_authority: Some(owner_pubkey.to_string()),
				extensions: vec![],
			}),
		);

		let mut mint_data = vec![0; mint_size];
		let mut mint_state =
			StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();

		let mint_close_authority = mint_state
			.init_extension::<MintCloseAuthority>(true)
			.unwrap();
		mint_close_authority.close_authority = Some(owner_pubkey).try_into().unwrap();
		mint_state.base = mint_base;
		mint_state.pack_base();
		mint_state.init_account_type().unwrap();

		assert_eq!(
			parse_token_v3(&mint_data, None).unwrap(),
			TokenAccountType::Mint(UiMint {
				mint_authority: Some(owner_pubkey.to_string()),
				supply: 42.to_string(),
				decimals: 3,
				is_initialized: true,
				freeze_authority: Some(owner_pubkey.to_string()),
				extensions: vec![UiExtension::MintCloseAuthority(UiMintCloseAuthority {
					close_authority: Some(owner_pubkey.to_string()),
				})],
			}),
		);
	}
}
