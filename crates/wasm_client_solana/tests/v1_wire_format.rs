//! Verifies the send path emits the SIMD-0385 wire format for every transaction
//! version.
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use solana_transaction::versioned::VersionedTransaction;
use wasm_client_solana::deserialize_and_decode;
use wasm_client_solana::serialize_and_encode;
use wasm_client_solana::solana_transaction_status::Encodable;
use wasm_client_solana::solana_transaction_status_client_types::EncodedTransaction;
use wasm_client_solana::solana_transaction_status_client_types::TransactionBinaryEncoding;
use wasm_client_solana::solana_transaction_status_client_types::UiTransactionEncoding;

fn wire_bytes(tx: &VersionedTransaction) -> Vec<u8> {
	let EncodedTransaction::Binary(b64, TransactionBinaryEncoding::Base64) =
		tx.encode(UiTransactionEncoding::Base64)
	else {
		panic!("expected base64 binary encoding");
	};
	BASE64_STANDARD.decode(&b64).unwrap()
}

fn v1_transaction() -> VersionedTransaction {
	let message = solana_message::v1::Message::new(
		solana_message::MessageHeader {
			num_required_signatures: 1,
			num_readonly_signed_accounts: 0,
			num_readonly_unsigned_accounts: 1,
		},
		solana_message::v1::TransactionConfig::empty(),
		solana_hash::Hash::new_from_array([7; 32]),
		vec![
			solana_pubkey::Pubkey::new_unique(),
			solana_pubkey::Pubkey::new_unique(),
		],
		vec![],
	);
	VersionedTransaction {
		signatures: vec![solana_signature::Signature::default()],
		message: solana_message::VersionedMessage::V1(message),
	}
}

fn v0_transaction() -> VersionedTransaction {
	VersionedTransaction {
		signatures: vec![solana_signature::Signature::default()],
		message: solana_message::VersionedMessage::V0(solana_message::v0::Message::default()),
	}
}

#[test]
fn send_path_emits_wire_format_for_v1() {
	let tx = v1_transaction();
	let encoded = serialize_and_encode(&tx, UiTransactionEncoding::Base64).unwrap();
	let sent = BASE64_STANDARD.decode(&encoded).unwrap();
	let wire = wire_bytes(&tx);

	assert_eq!(sent[0], solana_message::v1::V1_PREFIX);
	assert_eq!(sent, wire, "v1 send path must match SIMD-0385 wire bytes");
}

#[test]
fn send_path_emits_wire_format_for_v0() {
	let tx = v0_transaction();
	let encoded = serialize_and_encode(&tx, UiTransactionEncoding::Base64).unwrap();
	let sent = BASE64_STANDARD.decode(&encoded).unwrap();

	assert_eq!(sent, wire_bytes(&tx), "v0 send path must match wire bytes");
}

#[test]
fn send_path_round_trips_every_version() {
	for tx in [v0_transaction(), v1_transaction()] {
		let encoded = serialize_and_encode(&tx, UiTransactionEncoding::Base64).unwrap();
		let decoded: VersionedTransaction =
			deserialize_and_decode(&encoded, UiTransactionEncoding::Base64).unwrap();

		assert_eq!(decoded, tx);
	}
}
