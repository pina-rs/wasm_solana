#![cfg(feature = "ssr")]

use anyhow::Result;
use assert2::check;
use memory_wallet::MemoryWallet;
use solana_account::Account;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_message::VersionedMessage;
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_system_interface::instruction::transfer;
use solana_transaction::versioned::VersionedTransaction;
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
use wasm_client_solana::rpc_config::RpcTransactionConfig;
use wasm_client_solana::solana_transaction_status::UiTransactionEncoding;

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

/// A v1 transaction carries its compute budget in the message and places
/// signatures at the tail. Confirm the cluster accepts one built and sent
/// through this client.
///
/// A successful send is itself meaningful: `sendTransaction` runs preflight
/// simulation, so the node must have parsed the v1 wire bytes correctly before
/// returning a signature.
#[test(tokio::test(flavor = "multi_thread"))]
async fn sign_and_send_v1_transaction() -> Result<()> {
	let runner = create_runner().await;
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let lamports = sol_str_to_lamports("0.5").unwrap();
	let instruction = transfer(&pubkey, &target_pubkey, lamports);
	let rpc = runner.rpc().clone();
	let blockhash = rpc.get_latest_blockhash().await?;
	let transaction = VersionedTransaction::new_unsigned_v1(&pubkey, &[instruction], blockhash)?;

	check!(matches!(transaction.message, VersionedMessage::V1(_)));

	let mut memory_wallet = MemoryWallet::new(rpc.clone(), &[keypair]);

	memory_wallet.connect().await?;

	let props = SolanaSignAndSendTransactionProps::builder()
		.transaction(transaction)
		.build();
	let signature = memory_wallet.sign_and_send_transaction(props).await?;
	log::info!("v1 transaction successfully sent: {signature}");

	check!(signature != Signature::default());

	let confirmed = rpc
		.confirm_transaction_with_commitment(&signature, CommitmentConfig::confirmed())
		.await?;
	check!(confirmed);

	// The transfer must have actually executed, which means the cluster ran a
	// v1 transaction rather than merely accepting the bytes.
	let target = rpc
		.get_account_with_commitment(&target_pubkey, CommitmentConfig::confirmed())
		.await?;
	check!(target.is_some_and(|account| account.lamports == lamports));

	// The read path must be able to fetch it back. Request v1 explicitly, since
	// a v1 transaction is what `getTransaction` will not decode by default.
	let fetched = rpc
		.get_transaction_with_config(
			&signature,
			RpcTransactionConfig {
				encoding: Some(UiTransactionEncoding::Base64),
				commitment: Some(CommitmentConfig::confirmed()),
				..Default::default()
			},
		)
		.await?;
	let fetched_version = fetched
		.transaction
		.transaction
		.decode()
		.map(|tx| tx.message);

	check!(matches!(fetched_version, Some(VersionedMessage::V1(_))));

	Ok(())
}

/// A v1 message defaults its compute budget to zero, which the cluster rejects
/// with `MaxLoadedAccountsDataSizeExceeded`. The helper must set non-zero
/// limits so a caller does not have to know that.
#[test(tokio::test(flavor = "multi_thread"))]
async fn v1_helper_sets_non_zero_compute_budget() -> Result<()> {
	let runner = create_runner().await;
	let keypair = get_wallet_keypair();
	let pubkey = keypair.pubkey();
	let target_pubkey = Pubkey::new_unique();
	let instruction = transfer(&pubkey, &target_pubkey, sol_str_to_lamports("0.1").unwrap());
	let rpc = runner.rpc().clone();
	let blockhash = rpc.get_latest_blockhash().await?;
	let transaction = VersionedTransaction::new_unsigned_v1(&pubkey, &[instruction], blockhash)?;

	let VersionedMessage::V1(message) = &transaction.message else {
		panic!("expected a v1 message");
	};

	check!(message.config.compute_unit_limit.is_some_and(|v| v > 0));
	check!(
		message
			.config
			.loaded_accounts_data_size_limit
			.is_some_and(|v| v > 0)
	);

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
