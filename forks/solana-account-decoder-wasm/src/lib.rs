#![allow(clippy::arithmetic_side_effects)]
//! Wasm-compatible fork of Agave's `solana-account-decoder`.
//!
//! Decodes on-chain account data into the JSON shapes returned by the Solana
//! JSON RPC API. The `parse_*` modules cover the accounts of the standard
//! native programs and sysvars.

/// Dispatches account data to the parser for its owning program.
pub mod parse_account_data;
/// Parses address lookup table accounts.
pub mod parse_address_lookup_table;
/// Parses upgradeable and non-upgradeable BPF loader accounts.
pub mod parse_bpf_loader;
/// Parses accounts owned by the deprecated config program.
#[allow(deprecated)]
pub mod parse_config;
/// Parses nonce accounts.
pub mod parse_nonce;
/// Parses stake accounts.
pub mod parse_stake;
/// Parses sysvar accounts such as clock, rent, and stake history.
pub mod parse_sysvar;
/// Parses SPL Token and Token-2022 accounts.
pub mod parse_token;
/// Parses SPL Token-2022 extensions.
pub mod parse_token_extension;
/// Parses vote accounts.
pub mod parse_vote;
/// Parses the validator info account.
pub mod validator_info;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;
use serde::Serialize;
use solana_account::ReadableAccount;
pub use solana_account_decoder_client_types_wasm::UiAccount;
pub use solana_account_decoder_client_types_wasm::UiAccountData;
pub use solana_account_decoder_client_types_wasm::UiAccountEncoding;
pub use solana_account_decoder_client_types_wasm::UiDataSliceConfig;
use solana_fee_calculator::FeeCalculator;
use solana_pubkey::Pubkey;

use crate::parse_account_data::AccountAdditionalDataV3;
use crate::parse_account_data::parse_account_data_v3;

/// A numeric amount serialized as a string.
pub type StringAmount = String;
/// The number of decimals for an amount, serialized as a string.
pub type StringDecimals = String;
/// Maximum account data length, in bytes, that will be base58 encoded.
///
/// Larger data falls back to base64.
pub const MAX_BASE58_BYTES: usize = 128;

fn encode_bs58<T: ReadableAccount>(
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

/// Encode account data as a base58 string, applying an optional data slice.
///
/// Returns the error string `"error: data too large for bs58 encoding"` when
/// the resulting slice exceeds [`MAX_BASE58_BYTES`].
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
			let data = encode_bs58(account, data_slice_config);
			UiAccountData::LegacyBinary(data)
		}
		UiAccountEncoding::Base58 => {
			let data = encode_bs58(account, data_slice_config);
			UiAccountData::Binary(data, encoding)
		}
		UiAccountEncoding::Base64 => {
			UiAccountData::Binary(
				BASE64_STANDARD.encode(slice_data(account.data(), data_slice_config)),
				encoding,
			)
		}
		#[cfg(not(feature = "zstd"))]
		UiAccountEncoding::Base64Zstd => {
			// zstd support is optional in the wasm fork; fall back to plain
			// base64 rather than compiling native zstd on wasm targets.
			UiAccountData::Binary(
				BASE64_STANDARD.encode(slice_data(account.data(), data_slice_config)),
				UiAccountEncoding::Base64,
			)
		}
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

/// Account fee schedule as returned by RPC methods that report the current fee
/// calculator.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiFeeCalculator {
	/// Lamports charged per transaction signature, as a string.
	pub lamports_per_signature: StringAmount,
}

impl From<FeeCalculator> for UiFeeCalculator {
	fn from(fee_calculator: FeeCalculator) -> Self {
		Self {
			lamports_per_signature: fee_calculator.lamports_per_signature.to_string(),
		}
	}
}

impl Default for UiFeeCalculator {
	fn default() -> Self {
		Self {
			lamports_per_signature: "0".to_string(),
		}
	}
}

fn slice_data(data: &[u8], data_slice_config: Option<UiDataSliceConfig>) -> &[u8] {
	if let Some(UiDataSliceConfig { offset, length }) = data_slice_config {
		if offset >= data.len() {
			&[]
		} else if length > data.len() - offset {
			&data[offset..]
		} else {
			&data[offset..offset + length]
		}
	} else {
		data
	}
}

#[cfg(test)]
mod test {
	#[cfg(feature = "zstd")]
	use assert_matches::assert_matches;
	use solana_account::Account;
	use solana_account::AccountSharedData;

	use super::*;

	#[test]
	fn test_slice_data() {
		let data = vec![1, 2, 3, 4, 5];
		let slice_config = Some(UiDataSliceConfig {
			offset: 0,
			length: 5,
		});
		assert_eq!(slice_data(&data, slice_config), &data[..]);

		let slice_config = Some(UiDataSliceConfig {
			offset: 0,
			length: 10,
		});
		assert_eq!(slice_data(&data, slice_config), &data[..]);

		let slice_config = Some(UiDataSliceConfig {
			offset: 1,
			length: 2,
		});
		assert_eq!(slice_data(&data, slice_config), &data[1..3]);

		let slice_config = Some(UiDataSliceConfig {
			offset: 10,
			length: 2,
		});
		assert_eq!(slice_data(&data, slice_config), &[] as &[u8]);
	}

	#[test]
	fn test_encode_account_when_data_exceeds_base58_byte_limit() {
		let data = vec![42; MAX_BASE58_BYTES + 2];
		let account = AccountSharedData::from(Account {
			data,
			..Account::default()
		});

		// Whole account
		assert_eq!(
			encode_bs58(&account, None),
			"error: data too large for bs58 encoding"
		);

		// Slice of account that's still too large
		assert_eq!(
			encode_bs58(
				&account,
				Some(UiDataSliceConfig {
					length: MAX_BASE58_BYTES + 1,
					offset: 1
				})
			),
			"error: data too large for bs58 encoding"
		);

		// Slice of account that fits inside `MAX_BASE58_BYTES`
		assert_ne!(
			encode_bs58(
				&account,
				Some(UiDataSliceConfig {
					length: MAX_BASE58_BYTES,
					offset: 1
				})
			),
			"error: data too large for bs58 encoding"
		);

		// Slice of account that's too large, but whose intersection with the
		// account still fits
		assert_ne!(
			encode_bs58(
				&account,
				Some(UiDataSliceConfig {
					length: MAX_BASE58_BYTES + 1,
					offset: 2
				})
			),
			"error: data too large for bs58 encoding"
		);
	}

	#[test]
	#[cfg(feature = "zstd")]
	fn test_base64_zstd() {
		let encoded_account = encode_ui_account(
			&Pubkey::default(),
			&AccountSharedData::from(Account {
				data: vec![0; 1024],
				..Account::default()
			}),
			UiAccountEncoding::Base64Zstd,
			None,
			None,
		);
		assert_matches!(
			encoded_account.data,
			UiAccountData::Binary(_, UiAccountEncoding::Base64Zstd)
		);

		let decoded_account = encoded_account.to_account_shared_data().unwrap();
		assert_eq!(decoded_account.data(), &vec![0; 1024]);
		let decoded_account = encoded_account.to_account().unwrap();
		assert_eq!(decoded_account.data(), &vec![0; 1024]);
	}
}
