//! Hardened decoding of account data received over RPC.
//!
//! Account data arrives from the node as a string: base58 or base64 for
//! plain encodings, base64 wrapping a zstd stream for `base64+zstd`. The
//! upstream [`UiAccountData::decode`] maps those straight to bytes — which is
//! fine for the plain encodings, where the decoded size is bounded by the
//! payload that already crossed the wire, but a decompression bomb in the
//! zstd path can expand a few kilobytes of hostile RPC output into
//! gigabytes of memory.
//!
//! [`decode_account_data`] is the drop-in replacement this crate recommends:
//! identical behavior, except `base64+zstd` payloads are capped at
//! [`MAX_ACCOUNT_DATA_LEN`] — the same 10 MiB limit the cluster itself puts
//! on account data — and anything that expands past it is rejected instead
//! of truncating silently or exhausting memory.
//!
//! [`UiAccountData::decode`]: solana_account_decoder_client_types::UiAccountData::decode

use base64::Engine;
use base64::prelude::BASE64_STANDARD;

use crate::solana_account_decoder_client_types::UiAccountData;
use crate::solana_account_decoder_client_types::UiAccountEncoding;

/// The largest decompressed account payload this crate will produce.
///
/// Mirrors the cluster's own cap on account data size; an RPC node cannot
/// legitimately send anything larger.
pub const MAX_ACCOUNT_DATA_LEN: usize = 10 * 1024 * 1024;

/// Decode account data received over RPC into raw bytes.
///
/// Returns `None` for JSON-parsed payloads (their bytes are not on the
/// wire), for undecodable input, and — unlike the upstream
/// [`UiAccountData::decode`] — for `base64+zstd` payloads that decompress
/// beyond [`MAX_ACCOUNT_DATA_LEN`].
///
/// The zstd path is only compiled with the `zstd` feature; without it,
/// `base64+zstd` payloads return `None`, exactly as upstream does.
pub fn decode_account_data(data: &UiAccountData) -> Option<Vec<u8>> {
	match data {
		UiAccountData::Json(_) => None,
		UiAccountData::LegacyBinary(blob) => bs58::decode(blob).into_vec().ok(),
		UiAccountData::Binary(blob, encoding) => {
			match encoding {
				// Legacy labels that predate the explicit encodings: `binary`
				// means base58, `jsonParsed` never carries raw bytes.
				UiAccountEncoding::Binary | UiAccountEncoding::Base58 => {
					bs58::decode(blob).into_vec().ok()
				}
				UiAccountEncoding::JsonParsed => None,
				UiAccountEncoding::Base64 => BASE64_STANDARD.decode(blob).ok(),
				#[cfg(feature = "zstd")]
				UiAccountEncoding::Base64Zstd => {
					use std::io::Read as _;

					let zstd_data = BASE64_STANDARD.decode(blob).ok()?;
					let mut data = Vec::new();
					// Read one byte beyond the cap so oversized output is
					// rejected rather than silently truncated at the limit.
					let within_limit = zstd::stream::read::Decoder::new(zstd_data.as_slice())
						.map(|reader| reader.take(MAX_ACCOUNT_DATA_LEN as u64 + 1))
						.and_then(|mut reader| reader.read_to_end(&mut data))
						.is_ok_and(|_| data.len() <= MAX_ACCOUNT_DATA_LEN);
					within_limit.then_some(data)
				}
				#[cfg(not(feature = "zstd"))]
				UiAccountEncoding::Base64Zstd => None,
			}
		}
	}
}

#[cfg(test)]
mod tests {

	use assert2::check;

	use super::*;
	use crate::solana_account_decoder_client_types::ParsedAccount;

	fn binary(blob: &str, encoding: UiAccountEncoding) -> UiAccountData {
		UiAccountData::Binary(blob.to_owned(), encoding)
	}

	#[test]
	fn decodes_base64() {
		let data = binary("aGVsbG8gd29ybGQ=", UiAccountEncoding::Base64);

		check!(decode_account_data(&data) == Some(b"hello world".to_vec()));
	}

	#[test]
	fn rejects_malformed_base64() {
		let data = binary("**** not base64 ****", UiAccountEncoding::Base64);

		check!(decode_account_data(&data).is_none());
	}

	#[test]
	fn decodes_base58() {
		let data = binary("StV1DL6CwTryKyV", UiAccountEncoding::Base58);

		check!(decode_account_data(&data) == Some(b"hello world".to_vec()));
	}

	#[test]
	fn rejects_malformed_base58() {
		let data = binary("0OIl", UiAccountEncoding::Base58);

		check!(decode_account_data(&data).is_none());
	}

	#[test]
	fn decodes_legacy_base58() {
		let data = UiAccountData::LegacyBinary("StV1DL6CwTryKyV".to_owned());

		check!(decode_account_data(&data) == Some(b"hello world".to_vec()));
	}

	#[test]
	fn legacy_binary_label_decodes_as_base58() {
		let data = binary("StV1DL6CwTryKyV", UiAccountEncoding::Binary);

		check!(decode_account_data(&data) == Some(b"hello world".to_vec()));
	}

	#[test]
	fn json_parsed_label_has_no_bytes() {
		let data = binary("{}", UiAccountEncoding::JsonParsed);

		check!(decode_account_data(&data).is_none());
	}

	#[test]
	fn json_parsed_has_no_bytes() {
		let data = UiAccountData::Json(ParsedAccount {
			program: "nonce".to_owned(),
			parsed: serde_json::json!({}),
			space: 0,
		});

		check!(decode_account_data(&data).is_none());
	}

	#[cfg(feature = "zstd")]
	fn zstd_payload(raw: &[u8]) -> UiAccountData {
		let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 3).unwrap();
		use std::io::Write as _;
		encoder.write_all(raw).unwrap();
		let compressed = encoder.finish().unwrap();

		binary(
			&BASE64_STANDARD.encode(compressed),
			UiAccountEncoding::Base64Zstd,
		)
	}

	#[cfg(feature = "zstd")]
	#[test]
	fn decodes_zstd_within_limit() {
		let raw = vec![7u8; 1024];
		let data = zstd_payload(&raw);

		check!(decode_account_data(&data) == Some(raw));
	}

	#[cfg(feature = "zstd")]
	#[test]
	fn rejects_zstd_decompression_bomb() {
		// ~11 MiB of zeros compresses to a few kilobytes: a hostile node can
		// hand this to any client that trusts the decompressor.
		let raw = vec![0u8; MAX_ACCOUNT_DATA_LEN + 1];
		let data = zstd_payload(&raw);

		check!(decode_account_data(&data).is_none());
	}

	#[cfg(not(feature = "zstd"))]
	#[test]
	fn zstd_requires_the_feature() {
		let data = binary("aGVsbG8=", UiAccountEncoding::Base64Zstd);

		check!(decode_account_data(&data).is_none());
	}
}
