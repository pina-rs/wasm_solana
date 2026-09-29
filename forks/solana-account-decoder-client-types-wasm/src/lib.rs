//! Core RPC client types for solana-account-decoder
#![cfg_attr(docsrs, feature(doc_cfg))]
#[cfg(feature = "zstd")]
use std::io::Read;
use std::sync::Arc;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_with::DisplayFromStr;
use serde_with::serde_as;
use serde_with::skip_serializing_none;
use solana_account::Account;
use solana_account::AccountSharedData;
use solana_pubkey::Pubkey;
use typed_builder::TypedBuilder;

pub mod token;

/// A duplicate representation of an Account for pretty JSON serialization
#[serde_as]
#[skip_serializing_none]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct UiAccount {
	pub lamports: u64,
	pub data: UiAccountData,
	#[serde_as(as = "DisplayFromStr")]
	pub owner: Pubkey,
	pub executable: bool,
	pub rent_epoch: u64,
	#[builder(default, setter(into, strip_option))]
	pub space: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum UiAccountData {
	LegacyBinary(String), // Legacy. Retained for RPC backwards compatibility
	Json(ParsedAccount),
	Binary(String, UiAccountEncoding),
}

impl UiAccountData {
	/// Returns decoded account data in binary format if possible
	pub fn decode(&self) -> Option<Vec<u8>> {
		match self {
			UiAccountData::Json(_) => None,
			UiAccountData::LegacyBinary(blob) => bs58::decode(blob).into_vec().ok(),
			UiAccountData::Binary(blob, encoding) => {
				match encoding {
					UiAccountEncoding::Base58 => bs58::decode(blob).into_vec().ok(),
					UiAccountEncoding::Base64 => BASE64_STANDARD.decode(blob).ok(),
					#[cfg(feature = "zstd")]
					UiAccountEncoding::Base64Zstd => {
						BASE64_STANDARD.decode(blob).ok().and_then(|zstd_data| {
							let mut data = vec![];
							// Cap the decompressed size: a tiny compressed
							// blob from a hostile RPC must not be able to
							// expand into gigabytes and OOM the process.
							// Solana itself caps account data at 10 MiB.
							const MAX_ACCOUNT_DATA_LEN: usize = 10 * 1024 * 1024;
							zstd::stream::read::Decoder::new(zstd_data.as_slice())
								.map(|reader| reader.take(MAX_ACCOUNT_DATA_LEN as u64))
								.and_then(|mut reader| reader.read_to_end(&mut data))
								.map(|_| data)
								.ok()
						})
					}
					#[cfg(not(feature = "zstd"))]
					UiAccountEncoding::Base64Zstd => None,
					UiAccountEncoding::Binary | UiAccountEncoding::JsonParsed => None,
				}
			}
		}
	}
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum UiAccountEncoding {
	Binary, // Legacy. Retained for RPC backwards compatibility
	Base58,
	Base64,
	JsonParsed,
	#[serde(rename = "base64+zstd")]
	Base64Zstd,
}

impl UiAccount {
	pub fn to_account_shared_data(&self) -> Option<AccountSharedData> {
		let data = Arc::new(self.data.decode()?);
		Some(AccountSharedData::create_from_existing_shared_data(
			self.lamports,
			data,
			self.owner,
			self.executable,
			self.rent_epoch,
		))
	}

	pub fn to_account(&self) -> Option<Account> {
		let data = self.data.decode()?;
		Some(Account {
			lamports: self.lamports,
			data,
			owner: self.owner,
			executable: self.executable,
			rent_epoch: self.rent_epoch,
		})
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedAccount {
	pub program: String,
	pub parsed: Value,
	pub space: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiDataSliceConfig {
	pub offset: usize,
	pub length: usize,
}
