#![cfg(feature = "ssr")]

use anyhow::Result;
use assert2::check;
use memory_wallet::MemoryWallet;
use solana_commitment_config::CommitmentConfig;
use solana_native_token::sol_str_to_lamports;
use solana_sdk::account::Account;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;
use solana_system_interface::instruction::transfer;
use test_log::test;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::ProgramTest;
use test_utils_solana::ProgramTestContext;
use test_utils_solana::TestValidatorRunner;
use test_utils_solana::TestValidatorRunnerProps;
use test_utils_solana::prelude::*;
use wallet_standard::SolanaSignAndSendTransactionProps;
use wallet_standard::SolanaSignTransactionProps;
use wasm_client_solana::LOCALNET;
use wasm_client_solana::SolanaRpcClient;

#[test(tokio::test(flavor = "multi_thread"))]
async fn sign_transaction() -> Result<()> {
	let runner = create_runner().await;
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let instruction = transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.5").unwrap());
	let blockhash = runner.rpc().get_latest_blockhash().await?;
	let rpc = runner.rpc().clone();
	let transaction =
		VersionedTransaction::new_unsigned_v0(&pubkey, &[instruction], &[], blockhash)?;
	let mut memory_wallet = MemoryWallet::new(rpc, &[keypair]);

	memory_wallet.connect().await?;

	let props = SolanaSignTransactionProps::builder()
		.transaction(transaction)
		.build();
	let signed_transaction = memory_wallet.sign_transaction(props).await?;

	check!(signed_transaction.is_signed());

	Ok(())
}

#[test(tokio::test(flavor = "multi_thread"))]
async fn sign_and_send_transaction() -> Result<()> {
	let runner = create_runner().await;
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let instruction = transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.5").unwrap());
	let rpc = runner.rpc().clone();
	let blockhash = rpc.get_latest_blockhash().await?;
	let transaction =
		VersionedTransaction::new_unsigned_v0(&pubkey, &[instruction], &[], blockhash)?;
	let mut memory_wallet = MemoryWallet::new(rpc, &[keypair]);

	memory_wallet.connect().await?;

	log::info!("sending transaction: {transaction:#?}");
	let props = SolanaSignAndSendTransactionProps::builder()
		.transaction(transaction)
		.build();
	let signature = memory_wallet.sign_and_send_transaction(props).await?;
	log::info!("transaction successfully sent: {signature}");

	check!(signature != Signature::default());

	Ok(())
}

#[test(tokio::test)]
async fn banks_client_process_transaction() -> Result<()> {
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let (mut ctx, rpc) = create_program_test().await;
	let mut wallet = MemoryWallet::new(rpc, &[keypair]);
	let instruction = transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.5").unwrap());

	wallet.connect().await?;

	let transaction =
		VersionedTransaction::new_unsigned_v0(&pubkey, &[instruction], &[], Hash::default())?;
	let props = SolanaSignAndSendTransactionProps::builder()
		.transaction(transaction)
		.build();
	let result = ctx
		.banks_client
		.wallet_sign_and_process_transaction(&wallet, props)
		.await?;

	log::info!("{result:#?}");

	Ok(())
}

#[test(tokio::test)]
async fn banks_client_simulate_transaction() -> Result<()> {
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let (mut ctx, rpc) = create_program_test().await;
	let mut wallet = MemoryWallet::new(rpc, &[keypair]);
	let instruction = transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.5").unwrap());

	wallet.connect().await?;

	let transaction =
		VersionedTransaction::new_unsigned_v0(&pubkey, &[instruction], &[], Hash::default())?;
	let props = SolanaSignAndSendTransactionProps::builder()
		.transaction(transaction)
		.build();
	let result = ctx
		.banks_client
		.wallet_sign_and_simulate_transaction(&wallet, props)
		.await?;

	log::info!("{result:#?}");

	Ok(())
}

async fn create_runner() -> TestValidatorRunner {
	let pubkey = get_wallet_keypair().pubkey();
	TestValidatorRunner::run(
		TestValidatorRunnerProps::builder()
			.pubkeys(vec![pubkey])
			.build(),
	)
	.await
}

async fn create_program_test() -> (ProgramTestContext, SolanaRpcClient) {
	let pubkey = get_wallet_keypair().pubkey();
	let mut program_test = ProgramTest::default();
	let rpc = SolanaRpcClient::new_with_commitment(LOCALNET, CommitmentConfig::finalized());

	program_test.add_account(
		pubkey,
		Account {
			lamports: sol_str_to_lamports("1.0").unwrap(),
			..Account::default()
		},
	);

	let ctx = program_test.start_with_context().await;

	(ctx, rpc)
}
