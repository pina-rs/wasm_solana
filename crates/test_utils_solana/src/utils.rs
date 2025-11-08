#![allow(clippy::too_many_arguments)]

use std::fmt::Display;

use async_trait::async_trait;
use borsh::BorshSerialize;
use chrono_humanize::Accuracy;
use chrono_humanize::HumanTime;
use chrono_humanize::Tense;
use solana_account::Account;
use solana_banks_client::BanksClient;
use solana_banks_client::BanksClientError;
use solana_banks_interface::BanksTransactionResultWithSimulation;
use solana_clock::Clock;
use solana_clock::Slot;
use solana_commitment_config::CommitmentLevel;
use solana_keypair::Keypair;
use solana_loader_v3_interface::state::UpgradeableLoaderState;
use solana_message::VersionedMessage;
use solana_message::v0;
use solana_native_token::sol_str_to_lamports;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;
use solana_program_test::BanksTransactionResultWithMetadata;
use solana_program_test::ProgramTest;
use solana_program_test::ProgramTestContext;
use solana_pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_signer::Signer;
use solana_sysvar::rent::Rent;
use solana_transaction::versioned::VersionedTransaction;
use spl_associated_token_account_interface::address::get_associated_token_address;
use wallet_standard::SolanaSignAndSendTransactionProps;
use wallet_standard::SolanaSignTransactionProps;
use wallet_standard::prelude::*;
use wasm_client_solana::prelude::*;

#[async_trait(?Send)]
pub trait BanksClientAsyncExtension {
	/// Sign the transaction with the provided wallet.
	async fn wallet_sign_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		props: SolanaSignTransactionProps,
	) -> WalletResult<VersionedTransaction>;
	/// Sign the transaction and process it via the current banks client.
	async fn wallet_sign_and_process_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		props: SolanaSignAndSendTransactionProps,
	) -> WalletResult<BanksTransactionResultWithMetadata>;
	/// Sign and imulate the transaction.
	async fn wallet_sign_and_simulate_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		props: SolanaSignAndSendTransactionProps,
	) -> WalletResult<BanksTransactionResultWithSimulation>;
}

pub fn into_wallet_error<T: Display>(error: T) -> WalletError {
	WalletError::External(error.to_string())
}

#[async_trait(?Send)]
impl BanksClientAsyncExtension for BanksClient {
	async fn wallet_sign_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		SolanaSignTransactionProps {
			mut transaction, ..
		}: SolanaSignTransactionProps,
	) -> WalletResult<VersionedTransaction> {
		let hash = self
			.get_latest_blockhash()
			.await
			.map_err(into_wallet_error)?;
		transaction.try_sign(&[wallet], Some(hash))?;

		Ok(transaction)
	}

	async fn wallet_sign_and_process_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		SolanaSignAndSendTransactionProps {
			transaction,
			chain,
			options,
			..
		}: SolanaSignAndSendTransactionProps,
	) -> WalletResult<BanksTransactionResultWithMetadata> {
		let transaction = self
			.wallet_sign_transaction(
				wallet,
				SolanaSignTransactionProps {
					transaction,
					chain,
					options: options.map(Into::into),
				},
			)
			.await?;

		let metadata = self
			.process_transaction_with_metadata(transaction)
			.await
			.map_err(into_wallet_error)?;

		Ok(metadata)
	}

	async fn wallet_sign_and_simulate_transaction<W: WalletSolana + Signer>(
		&mut self,
		wallet: &W,
		SolanaSignAndSendTransactionProps {
			transaction,
			chain,
			options,
			..
		}: SolanaSignAndSendTransactionProps,
	) -> WalletResult<BanksTransactionResultWithSimulation> {
		let transaction = self
			.wallet_sign_transaction(
				wallet,
				SolanaSignTransactionProps {
					transaction,
					chain,
					options: options.map(Into::into),
				},
			)
			.await?;

		let result = self
			.simulate_transaction(transaction)
			.await
			.map_err(into_wallet_error)?;

		Ok(result)
	}
}

pub trait ProgramTestExtension {
	/// Adds a requested number of account with initial balance of `1_000` SOL
	/// to the test environment
	fn generate_accounts(&mut self, number_of_accounts: u8) -> Vec<Keypair>;
	/// Add a rent-exempt account with some data to the test environment.
	fn add_account_with_data(
		&mut self,
		pubkey: Pubkey,
		owner: Pubkey,
		data: &[u8],
		executable: bool,
	);
	/// Adds an account with the given balance to the test environment.
	fn add_account_with_lamports(&mut self, pubkey: Pubkey, owner: Pubkey, lamports: u64);
	/// Adds a rent-exempt account with some Packable data to the test
	/// environment.
	fn add_account_with_packable<P: Pack>(&mut self, pubkey: Pubkey, owner: Pubkey, data: P);
	/// Adds a rent-exempt account with some Borsh-serializable to the test
	/// environment
	fn add_account_with_borsh<B: BorshSerialize>(&mut self, pubkey: Pubkey, owner: Pubkey, data: B);
	/// Adds an SPL Token Mint account to the test environment.
	fn add_token_mint(
		&mut self,
		pubkey: Pubkey,
		mint_authority: Option<Pubkey>,
		supply: u64,
		decimals: u8,
		freeze_authority: Option<Pubkey>,
	);
	/// Adds an SPL Token account to the test environment.
	fn add_token_account(
		&mut self,
		pubkey: Pubkey,
		mint: Pubkey,
		owner: Pubkey,
		amount: u64,
		delegate: Option<Pubkey>,
		is_native: Option<u64>,
		delegated_amount: u64,
		close_authority: Option<Pubkey>,
	);
	/// Adds an associated token account to the test environment.
	/// Returns the address of the created account.
	fn add_associated_token_account(
		&mut self,
		mint: Pubkey,
		owner: Pubkey,
		amount: u64,
		delegate: Option<Pubkey>,
		is_native: Option<u64>,
		delegated_amount: u64,
		close_authority: Option<Pubkey>,
	) -> Pubkey;
	/// Adds a BPF program to the test environment.
	/// The program is upgradeable if `Some` `program_authority` is provided.
	fn add_bpf_program(
		&mut self,
		program_name: &'static str,
		program_id: Pubkey,
		program_authority: Option<Pubkey>,
		process_instruction: Option<BuiltinFunctionWithContext>,
	);
	/// Adds a BPF program to the test environment.
	/// The program is upgradeable if `Some` `program_authority` and then
	/// providing the  program data account This is useful for those programs
	/// which the program data has to be a spefic one, if not, use
	/// [`ProgramTestExtension::add_bpf_program`]
	fn add_bpf_program_with_program_data(
		&mut self,
		program_name: &'static str,
		program_id: Pubkey,
		program_authority: Option<Pubkey>,
		program_data: Pubkey,
		process_instruction: Option<BuiltinFunctionWithContext>,
	);
}

impl ProgramTestExtension for ProgramTest {
	fn generate_accounts(&mut self, number_of_accounts: u8) -> Vec<Keypair> {
		let mut accounts: Vec<Keypair> = vec![];

		for _ in 0..number_of_accounts {
			let keypair = Keypair::new();
			let initial_lamports = sol_str_to_lamports("1000.0").unwrap();
			self.add_account_with_lamports(keypair.pubkey(), keypair.pubkey(), initial_lamports);
			accounts.push(keypair);
		}
		accounts
	}

	fn add_account_with_data(
		&mut self,
		pubkey: Pubkey,
		owner: Pubkey,
		data: &[u8],
		executable: bool,
	) {
		self.add_account(
			pubkey,
			Account {
				lamports: Rent::default().minimum_balance(data.len()),
				data: data.to_vec(),
				executable,
				owner,
				rent_epoch: 0,
			},
		);
	}

	fn add_account_with_lamports(&mut self, pubkey: Pubkey, owner: Pubkey, lamports: u64) {
		self.add_account(
			pubkey,
			Account {
				lamports,
				data: vec![],
				executable: false,
				owner,
				rent_epoch: 0,
			},
		);
	}

	fn add_account_with_packable<P: Pack>(&mut self, pubkey: Pubkey, owner: Pubkey, data: P) {
		let data = {
			let mut buf = vec![0u8; P::LEN];
			data.pack_into_slice(&mut buf[..]);
			buf
		};
		self.add_account_with_data(pubkey, owner, &data, false);
	}

	fn add_account_with_borsh<B: BorshSerialize>(
		&mut self,
		pubkey: Pubkey,
		owner: Pubkey,
		data: B,
	) {
		let mut destination = vec![];
		data.serialize(&mut destination)
			.expect("failed to serialize daat");
		self.add_account_with_data(pubkey, owner, &destination, false);
	}

	fn add_token_mint(
		&mut self,
		pubkey: Pubkey,
		mint_authority: Option<Pubkey>,
		supply: u64,
		decimals: u8,
		freeze_authority: Option<Pubkey>,
	) {
		self.add_account_with_packable(
			pubkey,
			spl_token_2022_interface::ID,
			spl_token_2022_interface::state::Mint {
				mint_authority: COption::from(mint_authority),
				supply,
				decimals,
				is_initialized: true,
				freeze_authority: COption::from(freeze_authority),
			},
		);
	}

	fn add_token_account(
		&mut self,
		pubkey: Pubkey,
		mint: Pubkey,
		owner: Pubkey,
		amount: u64,
		delegate: Option<Pubkey>,
		is_native: Option<u64>,
		delegated_amount: u64,
		close_authority: Option<Pubkey>,
	) {
		self.add_account_with_packable(
			pubkey,
			spl_token_2022_interface::id(),
			spl_token_2022_interface::state::Account {
				mint,
				owner,
				amount,
				delegate: COption::from(delegate),
				state: spl_token_2022_interface::state::AccountState::Initialized,
				is_native: COption::from(is_native),
				delegated_amount,
				close_authority: COption::from(close_authority),
			},
		);
	}

	fn add_associated_token_account(
		&mut self,
		mint: Pubkey,
		owner: Pubkey,
		amount: u64,
		delegate: Option<Pubkey>,
		is_native: Option<u64>,
		delegated_amount: u64,
		close_authority: Option<Pubkey>,
	) -> Pubkey {
		let pubkey = get_associated_token_address(&owner, &mint);
		self.add_token_account(
			pubkey,
			mint,
			owner,
			amount,
			delegate,
			is_native,
			delegated_amount,
			close_authority,
		);

		pubkey
	}

	fn add_bpf_program(
		&mut self,
		program_name: &'static str,
		program_id: Pubkey,
		program_authority: Option<Pubkey>,
		process_instruction: Option<BuiltinFunctionWithContext>,
	) {
		if let Some(program_authority) = program_authority {
			let program_file =
				solana_program_test::find_file(&format!("{program_name}.so")).unwrap();
			let program_bytes = solana_program_test::read_file(program_file.clone());
			let program_data_pubkey = Pubkey::new_unique();
			let mut program = Vec::<u8>::new();

			bincode::serialize_into(
				&mut program,
				&UpgradeableLoaderState::Program {
					programdata_address: program_data_pubkey,
				},
			)
			.unwrap();

			let mut program_data = Vec::<u8>::new();

			bincode::serialize_into(
				&mut program_data,
				&UpgradeableLoaderState::ProgramData {
					slot: 0,
					upgrade_authority_address: Some(program_authority),
				},
			)
			.unwrap();

			log::info!(
				"\"{}\" BPF program from {}{}",
				program_name,
				program_file.display(),
				std::fs::metadata(&program_file)
					.map(|metadata| {
						metadata
							.modified()
							.map(|time| {
								format!(
									", modified {}",
									HumanTime::from(time)
										.to_text_en(Accuracy::Precise, Tense::Past)
								)
							})
							.ok()
					})
					.ok()
					.flatten()
					.unwrap_or_default()
			);

			self.add_account_with_data(
				program_id,
				bpf_loader_upgradeable::id(),
				program.as_ref(),
				true,
			);

			self.add_account_with_data(
				program_data_pubkey,
				bpf_loader_upgradeable::id(),
				&[program_data.as_slice(), program_bytes.as_slice()].concat(),
				false,
			);
		} else {
			self.add_program(program_name, program_id, process_instruction);
		}
	}

	fn add_bpf_program_with_program_data(
		&mut self,
		program_name: &'static str,
		program_id: Pubkey,
		program_authority: Option<Pubkey>,
		program_data_pubkey: Pubkey,
		process_instruction: Option<BuiltinFunctionWithContext>,
	) {
		if let Some(program_authority) = program_authority {
			let program_file =
				solana_program_test::find_file(&format!("{program_name}.so")).unwrap();
			let program_bytes = solana_program_test::read_file(program_file.clone());

			let mut program = Vec::<u8>::new();
			bincode::serialize_into(
				&mut program,
				&UpgradeableLoaderState::Program {
					programdata_address: program_data_pubkey,
				},
			)
			.unwrap();

			let mut program_data = Vec::<u8>::new();
			bincode::serialize_into(
				&mut program_data,
				&UpgradeableLoaderState::ProgramData {
					slot: 0,
					upgrade_authority_address: Some(program_authority),
				},
			)
			.unwrap();

			log::info!(
				"\"{}\" BPF program from {}{}",
				program_name,
				program_file.display(),
				std::fs::metadata(&program_file)
					.map(|metadata| {
						metadata
							.modified()
							.map(|time| {
								format!(
									", modified {}",
									HumanTime::from(time)
										.to_text_en(Accuracy::Precise, Tense::Past)
								)
							})
							.ok()
					})
					.ok()
					.flatten()
					.unwrap_or_default()
			);

			self.add_account_with_data(
				program_id,
				bpf_loader_upgradeable::id(),
				program.as_ref(),
				true,
			);

			self.add_account_with_data(
				program_data_pubkey,
				bpf_loader_upgradeable::id(),
				&[program_data.as_slice(), program_bytes.as_slice()].concat(),
				false,
			);
		} else {
			self.add_program(program_name, program_id, process_instruction);
		}
	}
}

#[async_trait(?Send)]
pub trait ProgramTestContextExtension {
	/// Create an adhoc funded keypair address.
	async fn create_funded_keypair(&mut self) -> Result<Keypair, BanksClientError>;
	/// Get the current slot
	async fn get_slot(&mut self) -> Result<Slot, BanksClientError>;
	/// Fund an account with the set number of lamports.
	async fn fund_account(
		&mut self,
		address: &Pubkey,
		lamports: u64,
	) -> Result<(), BanksClientError>;
	/// Calculate slot number from the provided unix timestamp in seconds.
	async fn warp_to_timestamp(&mut self, timestamp: i64) -> Result<(), BanksClientError>;
	/// Jump forward by the provided number of seconds.
	async fn fast_forward(&mut self, number_of_seconds: i64) -> Result<(), BanksClientError>;
}

#[async_trait(?Send)]
impl ProgramTestContextExtension for ProgramTestContext {
	async fn create_funded_keypair(&mut self) -> Result<Keypair, BanksClientError> {
		let keypair = Keypair::new();
		let instruction = solana_system_interface::instruction::transfer(
			&self.payer.pubkey(),
			&keypair.pubkey(),
			sol_str_to_lamports("10.0").unwrap(),
		);
		let hash = self.banks_client.get_latest_blockhash().await?;
		let message = v0::Message::try_compile(&self.payer.pubkey(), &[instruction], &[], hash)
			.map_err(|_| BanksClientError::ClientError("could not compile message"))?;
		let versioned_message = VersionedMessage::V0(message);
		let transaction = VersionedTransaction::try_new(versioned_message, &[&self.payer])
			.map_err(|_| {
				BanksClientError::ClientError("could not sign the versioned transaction")
			})?;
		self.banks_client.process_transaction(transaction).await?;

		Ok(keypair)
	}

	async fn get_slot(&mut self) -> Result<Slot, BanksClientError> {
		self.banks_client
			.get_slot_with_context(tarpc::context::current(), CommitmentLevel::Finalized)
			.await
	}

	async fn fund_account(
		&mut self,
		address: &Pubkey,
		lamports: u64,
	) -> Result<(), BanksClientError> {
		if let Some(account) = self
			.banks_client
			.get_account_with_commitment(*address, CommitmentLevel::Finalized)
			.await?
		{
			let lamports = account.lamports + lamports;
			let updated_account = Account {
				lamports,
				..account
			}
			.into();
			self.set_account(address, &updated_account);
		} else {
			let new_account = Account {
				lamports,
				..Account::default()
			}
			.into();
			self.set_account(address, &new_account);
		}

		Ok(())
	}

	async fn warp_to_timestamp(&mut self, timestamp: i64) -> Result<(), BanksClientError> {
		const NANOSECONDS_IN_SECOND: i64 = 1_000_000_000;

		let mut clock: Clock = self.banks_client.get_sysvar().await?;
		let now = clock.unix_timestamp;
		let current_slot = clock.slot;
		clock.unix_timestamp = timestamp;

		if now >= timestamp {
			return Err(BanksClientError::ClientError("Warp slot not in the future"));
		}

		let nanoseconds_per_slot = self.genesis_config().ns_per_slot();
		let timestamp_diff_nanoseconds = timestamp
			.checked_sub(now) // calculate time diff
			.expect("Problem with timestamp diff calculation.")
			.checked_mul(NANOSECONDS_IN_SECOND) // convert from s to ns
			.expect("Problem with timestamp diff calculation.")
			as u128;

		let slots = timestamp_diff_nanoseconds
			.checked_div(nanoseconds_per_slot)
			.expect("Problem with slots from timestamp calculation.") as u64;

		self.set_sysvar(&clock);
		self.warp_to_slot(current_slot + slots)
			.map_err(|_| BanksClientError::ClientError("Warp slot not in the future"))?;

		Ok(())
	}

	async fn fast_forward(&mut self, number_of_seconds: i64) -> Result<(), BanksClientError> {
		let clock: Clock = self.banks_client.get_sysvar().await?;

		self.warp_to_timestamp(
			clock
				.unix_timestamp
				.checked_add(number_of_seconds)
				.expect("Number of seconds added is too great"),
		)
		.await
	}
}
