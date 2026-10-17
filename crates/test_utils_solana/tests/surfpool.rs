//! End-to-end client tests against an embedded surfpool node: dynamic-port
//! transports, startup funding, airdrops, and confirmation riding the
//! `signatureSubscribe` notification.
#![cfg(feature = "surfpool")]

use assert2::check;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use solana_signer::Signer;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::SurfpoolRunner;
use test_utils_solana::SurfpoolRunnerProps;

async fn create_runner() -> SurfpoolRunner {
	let pubkey = get_wallet_keypair().pubkey();
	SurfpoolRunnerProps::builder()
		.pubkeys(vec![pubkey])
		.initial_lamports(sol_str_to_lamports("10.0").unwrap())
		.build()
		.run()
		.await
}

/// Startup funding lands on every requested pubkey and on the payer, over
/// the dynamically bound HTTP endpoint.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn funds_pubkeys_and_payer_at_startup() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();

	let wallet = rpc.get_balance(&get_wallet_keypair().pubkey()).await?;
	check!(wallet == sol_str_to_lamports("10.0").unwrap());

	let payer = rpc.get_balance(&runner.payer().pubkey()).await?;
	check!(payer == sol_str_to_lamports("10.0").unwrap());

	Ok(())
}

/// The full send path — airdrop, blockhash, sign, send, confirm — over both
/// transports on surfpool's dynamic ports: HTTP for the sends, websocket
/// pubsub for the confirmation notification.
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn send_and_confirm_via_pubsub() -> anyhow::Result<()> {
	let runner = create_runner().await;
	let rpc = runner.rpc().clone();
	let recipient = Keypair::new().pubkey();

	let signature = rpc
		.request_airdrop(&recipient, sol_str_to_lamports("1.0").unwrap())
		.await?;
	check!(rpc.confirm_transaction(&signature).await?);

	let instruction = solana_system_interface::instruction::transfer(
		&runner.payer().pubkey(),
		&recipient,
		sol_str_to_lamports("0.1").unwrap(),
	);
	let recent_blockhash = rpc.get_latest_blockhash().await?;
	let transaction = solana_transaction::Transaction::new_signed_with_payer(
		&[instruction],
		Some(&runner.payer().pubkey()),
		&[runner.payer()],
		recent_blockhash,
	);

	rpc.send_and_confirm_transaction(&transaction.into())
		.await?;

	let balance = rpc.get_balance(&recipient).await?;
	check!(balance == sol_str_to_lamports("1.1").unwrap());

	Ok(())
}
