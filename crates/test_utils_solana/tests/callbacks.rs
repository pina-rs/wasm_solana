//! Callback-driven subscription tests against a local validator, the
//! callback counterparts of the stream tests in `streams.rs`.
#![cfg(feature = "test_validator")]

use std::time::Duration;

use assert2::check;
use futures::StreamExt;
use futures::channel::mpsc;
use solana_commitment_config::CommitmentConfig;
use solana_keypair::Keypair;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;
use test_utils_solana::prelude::*;
use tokio::time::timeout;
use wasm_client_solana::SlotSubscribeRequest;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::rpc_config::LogsSubscribeRequest;
use wasm_client_solana::rpc_config::RpcTransactionLogsConfig;
use wasm_client_solana::rpc_config::RpcTransactionLogsFilter;

/// The `on_logs` callback receives the notification payloads a
/// `logs_subscribe` stream would yield, without the test driving the stream.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn log_callback_subscription() -> anyhow::Result<()> {
	let runner = create_runner().await;
	// Confirmed, not the runner's finalized default: finalized vote logs can
	// take longer than this test's notification budget on a loaded CI
	// runner, and the test observes delivery, not finality.
	let rpc = SolanaRpcClient::new_with_ws_and_commitment(
		&runner.rpc_url(),
		&runner.pubsub_url(),
		CommitmentConfig::confirmed(),
	);
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = rpc
		.on_logs(
			LogsSubscribeRequest::builder()
				.filter(RpcTransactionLogsFilter::AllWithVotes)
				// Confirmed inside the subscription: without it the node's
				// finalized default gates notifications, which a loaded CI
				// runner can push past this test's budget.
				.config(RpcTransactionLogsConfig {
					commitment: Some(CommitmentConfig::confirmed()),
				})
				.build(),
			move |notification| {
				let _ = sender.unbounded_send(notification);
			},
		)
		.await?;

	check!(subscription.subscription_id() == 0);
	check!(!subscription.is_stopped());

	let notification = timeout(Duration::from_secs(30), receiver.next())
		.await
		.unwrap()
		.unwrap();

	check!(!notification.value.logs.is_empty());

	subscription.unsubscribe().await?;

	Ok(())
}

/// The `on_slot_change` callback keeps receiving slots until the handle is
/// unsubscribed; the payloads arrive in order.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn slot_callback_subscription_receives_ordered_slots() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = rpc
		.on_slot_change(SlotSubscribeRequest::builder().build(), move |slot| {
			let _ = sender.unbounded_send(slot);
		})
		.await?;

	let mut slots = Vec::new();
	for _ in 0..3 {
		slots.push(
			timeout(Duration::from_secs(30), receiver.next())
				.await
				.unwrap()
				.unwrap(),
		);
	}

	check!(slots.iter().all(|slot| slot.slot > 0));
	check!(slots.windows(2).all(|pair| pair[0].slot < pair[1].slot));

	subscription.unsubscribe().await?;

	Ok(())
}

/// Dropping the handle stops the pump: a feed that keeps producing (slots
/// arrive every block) must stop invoking the callback once the handle is
/// gone.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn dropping_the_handle_stops_the_callback() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let received = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

	let subscription = rpc
		.on_slot_change(SlotSubscribeRequest::builder().build(), {
			let received = std::sync::Arc::clone(&received);
			move |_| {
				received.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			}
		})
		.await?;

	// Slots arrive roughly every block; wait long enough for a healthy
	// stream to have delivered several, then prove the drop stopped it.
	tokio::time::sleep(Duration::from_secs(2)).await;
	let before = received.load(std::sync::atomic::Ordering::Relaxed);
	drop(subscription);
	tokio::time::sleep(Duration::from_secs(2)).await;
	let after = received.load(std::sync::atomic::Ordering::Relaxed);

	check!(before > 0);
	check!(after == before);

	Ok(())
}

/// The `on_account_change` callback observes an account being created, the
/// callback counterpart of `account_stream_subscription` in `streams.rs`.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn account_callback_subscription() -> anyhow::Result<()> {
	let runner = create_runner().await;
	// The test needs the transaction landed, not finalized: on a loaded CI
	// runner, waiting out the finalized commitment can exceed the send-and-
	// confirm budget, and finality adds nothing to what this test observes.
	let rpc = SolanaRpcClient::new_with_ws_and_commitment(
		&runner.rpc_url(),
		&runner.pubsub_url(),
		CommitmentConfig::confirmed(),
	);
	let new_account = Keypair::new();
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = rpc
		.on_account_change(new_account.pubkey(), move |notification| {
			let _ = sender.unbounded_send(notification);
		})
		.await?;

	// Create a transaction to create an account with custom data
	let payer = get_wallet_keypair();
	let space: u64 = 100; // Size of the account data
	let lamports = rpc
		.get_minimum_balance_for_rent_exemption(space as usize)
		.await?;

	let instruction = solana_system_interface::instruction::create_account(
		&payer.pubkey(),
		&new_account.pubkey(),
		lamports,
		space,
		&solana_sdk_ids::system_program::id(),
	);
	let recent_blockhash = rpc.get_latest_blockhash().await.unwrap();
	let transaction = solana_transaction::Transaction::new_signed_with_payer(
		&[instruction],
		Some(&payer.pubkey()),
		&[&payer, &new_account],
		recent_blockhash,
	);
	rpc.send_and_confirm_transaction(&transaction.into())
		.await
		.unwrap();

	let notification = timeout(Duration::from_secs(90), receiver.next())
		.await
		.unwrap()
		.unwrap();
	let value = notification.value.unwrap();

	check!(value.space == Some(space));

	subscription.unsubscribe().await?;

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
