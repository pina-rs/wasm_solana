#![cfg(feature = "js")]

use anyhow::Result;
use assert2::check;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use test_utils_keypairs::get_wallet_keypair;
use wasm_bindgen_test::*;
use wasm_client_solana::LOCALNET;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::prelude::*;
use wasm_client_solana::rpc_config::LogsSubscribeRequest;
use wasm_client_solana::rpc_config::RpcTransactionLogsFilter;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
pub async fn request_airdrop() -> Result<()> {
	let rpc = SolanaRpcClient::new(LOCALNET);
	let pubkey = Keypair::new().pubkey();
	let lamports = sol_str_to_lamports("1.0").unwrap();
	let signature = rpc.request_airdrop(&pubkey, lamports).await?;
	rpc.confirm_transaction(&signature).await?;

	let account = rpc.get_account(&pubkey).await?;

	check!(account.lamports == lamports);

	Ok(())
}

#[wasm_bindgen_test]
pub async fn log_subscription() -> Result<()> {
	let rpc = SolanaRpcClient::new(LOCALNET);
	let subscription = rpc
		.logs_subscribe(
			LogsSubscribeRequest::builder()
				.filter(RpcTransactionLogsFilter::AllWithVotes)
				.build(),
		)
		.await?;
	let unsubscription = subscription.get_unsubscription();
	let mut stream2 = subscription.take(2);

	while let Some(log_notification_request) = stream2.next().await {
		console_log!("log: {log_notification_request:#?}");
		check!(log_notification_request.method == "logsNotification");
	}

	unsubscription.run().await?;

	Ok(())
}

// TODO this test doesn't actually work. Spent too long trying to get it to
// fail for the correct reason. It seems like there is a lock somewhere that is
// only released on drop. So when the subscription is dropped all the stream
// updates are processed, but nothing happens in the subscription since it has
// already been dropped.
#[wasm_bindgen_test]
pub async fn account_subscription() -> Result<()> {
	let date = js_sys::Date::new_0();
	console_log!("initial: {}", date.to_iso_string());
	let rpc = SolanaRpcClient::new(LOCALNET);
	let new_account = Keypair::new();
	let mut subscription = rpc.account_subscribe(new_account.pubkey()).await?;
	let unsubscription = subscription.get_unsubscription();
	let space: u64 = 100; // Size of the account data
	let lamports = rpc
		.get_minimum_balance_for_rent_exemption(space as usize)
		.await?;
	let elapsed = js_sys::Date::now() - date.get_time();
	console_log!("elapsed 1: {elapsed}");

	wasm_bindgen_futures::spawn_local(async move {
		console_log!("inside spawn_local");
		while let Some(account_notification) = subscription.next().await {
			console_log!("account: {account_notification:#?}");
			let value = account_notification.params.result.value.unwrap();
			check!(value.space == Some(space));
			check!(value.lamports == lamports);
		}
	});

	let payer = get_wallet_keypair();
	let instruction = solana_system_interface::instruction::create_account(
		&payer.pubkey(),
		&new_account.pubkey(),
		lamports,
		space,
		&solana_sdk_ids::system_program::id(),
	);
	let signature = rpc
		.request_airdrop(&payer.pubkey(), sol_str_to_lamports("1.0").unwrap())
		.await?;
	rpc.confirm_transaction(&signature).await?;

	let elapsed = js_sys::Date::now() - date.get_time();
	console_log!("airdrop 1 signature: {signature}");
	console_log!("elapsed 2: {elapsed}");

	let recent_blockhash = rpc.get_latest_blockhash().await.unwrap();
	let transaction = solana_transaction::Transaction::new_signed_with_payer(
		&[instruction],
		Some(&payer.pubkey()),
		&[&payer, &new_account],
		recent_blockhash,
	);

	let signature = rpc
		.send_and_confirm_transaction(&transaction.into())
		.await?;
	rpc.confirm_transaction(&signature).await?;
	let elapsed = js_sys::Date::now() - date.get_time();
	console_log!("elapsed 3: {elapsed}");

	let signature = rpc
		.request_airdrop(&new_account.pubkey(), sol_str_to_lamports("1.0").unwrap())
		.await?;
	rpc.confirm_transaction(&signature).await?;
	let elapsed = js_sys::Date::now() - date.get_time();
	console_log!("airdrop 2 signature: {signature}");
	console_log!("elapsed 4: {elapsed}");

	let signature = rpc
		.request_airdrop(&new_account.pubkey(), sol_str_to_lamports("1.0").unwrap())
		.await?;
	rpc.confirm_transaction(&signature).await?;
	let elapsed = js_sys::Date::now() - date.get_time();
	console_log!("airdrop 3 signature: {signature}");
	console_log!("elapsed 5: {elapsed}");

	unsubscription.run().await?;

	Ok(())
}
