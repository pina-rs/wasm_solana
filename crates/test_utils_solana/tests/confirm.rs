//! Confirmation behavior against a live validator: the polling fallback
//! when the pubsub endpoint is unavailable, and the error surfaced by
//! `send_and_confirm_transaction` for a transaction that lands with an
//! error.
#![cfg(feature = "test_validator")]

use std::time::Duration;

use assert2::check;
use solana_commitment_config::CommitmentConfig;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;
use test_utils_solana::prelude::*;
use tokio::time::timeout;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::rpc_config::RpcSendTransactionConfig;
use wasm_client_solana::solana_transaction_status::UiTransactionEncoding;

async fn create_runner() -> TestValidatorRunner {
	let pubkey = get_wallet_keypair().pubkey();
	TestValidatorRunnerProps::builder()
		.pubkeys(vec![pubkey])
		.build()
		.run()
		.await
}

/// Confirming must survive a pubsub endpoint that refuses connections: the
/// signature subscription fails, the polling fallback takes over, and the
/// airdrop still confirms within one polling budget.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn confirms_via_polling_when_pubsub_is_unavailable() -> anyhow::Result<()> {
	let runner = create_runner().await;
	// Port 1 has no listener on a test machine; the explicit `ws://` scheme
	// passes through `get_ws_url` untouched, pointing pubsub at the dead
	// port while HTTP keeps using the validator.
	let rpc = SolanaRpcClient::new_with_ws_and_commitment(
		&runner.rpc_url(),
		"ws://127.0.0.1:1",
		CommitmentConfig::confirmed(),
	);
	let signature = rpc
		.request_airdrop(
			&Keypair::new().pubkey(),
			sol_str_to_lamports("1.0").unwrap(),
		)
		.await?;

	// One polling budget (MAX_RETRIES * SLEEP_MS) plus slack for the failed
	// subscription attempt.
	let confirmed = timeout(Duration::from_secs(30), rpc.confirm_transaction(&signature))
		.await
		.expect("confirmation within the polling budget")?;

	check!(confirmed);

	Ok(())
}

/// A transaction that lands on-chain with an error must surface as a
/// confirmation failure: `send_and_confirm_transaction` errors carrying the
/// signature instead of silently reporting success. The transfer is sent
/// with the preflight skipped so the send itself succeeds and the error is
/// delivered by confirmation.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn send_and_confirm_errors_for_failed_transaction() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let payer = Keypair::new();

	// Enough for fees, nowhere near the transferred amount, so the
	// transaction is banked with an error rather than rejected outright.
	rpc.request_airdrop(&payer.pubkey(), sol_str_to_lamports("0.01").unwrap())
		.await?;

	let instruction = solana_system_interface::instruction::transfer(
		&payer.pubkey(),
		&Keypair::new().pubkey(),
		sol_str_to_lamports("5.0").unwrap(),
	);
	let recent_blockhash = rpc.get_latest_blockhash().await?;
	let transaction = solana_transaction::Transaction::new_signed_with_payer(
		&[instruction],
		Some(&payer.pubkey()),
		&[&payer],
		recent_blockhash,
	);

	let signature = transaction.signatures[0];
	let error = rpc
		.send_and_confirm_transaction_with_config(
			&transaction.into(),
			CommitmentConfig::confirmed(),
			RpcSendTransactionConfig {
				skip_preflight: true,
				preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
				encoding: Some(UiTransactionEncoding::Base64),
				..RpcSendTransactionConfig::default()
			},
		)
		.await
		.expect_err("a failed transaction cannot confirm");

	let message = error.to_string();
	check!(message.contains(&signature.to_string()));

	Ok(())
}
