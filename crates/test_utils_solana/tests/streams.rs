#![cfg(feature = "test_validator")]

//! Tests for the `solana_client_wasm` crate are placed here since this depends
//! on `wasm_client_solana`.

use std::time::Duration;

use assert2::check;
use solana_sdk::signature::Keypair;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;
use test_utils_solana::prelude::*;
use tokio::time::timeout;
use wasm_client_solana::rpc_config::LogsSubscribeRequest;
use wasm_client_solana::rpc_config::RpcTransactionLogsFilter;

#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn log_stream_subscription() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let subscription = rpc
		.logs_subscribe(
			LogsSubscribeRequest::builder()
				.filter(RpcTransactionLogsFilter::AllWithVotes)
				.build(),
		)
		.await?;

	check!(subscription.subscription_id() == 0);

	let mut stream5 = subscription.clone().take(5);

	while let Some(log_notification_request) = stream5.next().await {
		check!(log_notification_request.method == "logsNotification");
	}

	subscription.unsubscribe().await?;

	Ok(())
}

#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn account_stream_subscription() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let new_account = Keypair::new();
	let pubkey = new_account.pubkey();
	let mut subscription = rpc.account_subscribe(pubkey).await?;
	let unsubscription = subscription.get_unsubscription();
	let next_update = subscription.next();

	// Create a transaction to create an account with custom data
	let payer = get_wallet_keypair();
	let space = 100; // Size of the account data
	let lamports = rpc.get_minimum_balance_for_rent_exemption(space).await?;

	let instruction = solana_system_interface::instruction::create_account(
		&payer.pubkey(),
		&pubkey,
		lamports,
		space as u64,
		&solana_sdk_ids::system_program::id(),
	);
	let recent_blockhash = rpc.get_latest_blockhash().await.unwrap();
	let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
		&[instruction],
		Some(&payer.pubkey()),
		&[&payer, &new_account],
		recent_blockhash,
	);
	rpc.send_and_confirm_transaction(&transaction.into())
		.await
		.unwrap();

	let next_update_with_timeout = timeout(Duration::from_secs(30), next_update);
	let account_info = next_update_with_timeout.await.unwrap().unwrap();

	check!(account_info.method == "accountNotification");
	check!(account_info.params.result.value.unwrap().space == Some(100));

	unsubscription.run().await?;

	Ok(())
}

async fn create_runner() -> TestValidatorRunner {
	let pubkey = get_wallet_keypair().pubkey();
	TestValidatorRunnerProps::builder()
		.pubkeys(vec![pubkey])
		.build()
		.run()
		.await
}
