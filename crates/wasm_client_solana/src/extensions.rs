#![allow(clippy::manual_async_fn)]

use std::future::Future;
use std::ops::Div;
use std::ops::Mul;

use solana_address_lookup_table_interface::instruction::create_lookup_table;
use solana_address_lookup_table_interface::instruction::extend_lookup_table;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::AddressLookupTableAccount;
use solana_message::CompileError;
use solana_message::VersionedMessage;
use solana_message::v0;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_signer::SignerError;
use solana_signer::signers::Signers;
use solana_transaction::versioned::VersionedTransaction;
use wallet_standard::SolanaSignTransactionOptions;
use wallet_standard::SolanaSignTransactionOutput;
use wallet_standard::SolanaSignTransactionProps;
use wallet_standard::SolanaSignatureOutput;
use wallet_standard::WalletResult;
use wallet_standard::WalletSolanaPubkey;
use wallet_standard::WalletSolanaSignMessage;
use wallet_standard::WalletSolanaSignTransaction;

use crate::COMPUTE_UNIT_MAX_LIMIT;
use crate::ClientError;
use crate::ClientResult;
use crate::MAX_LOOKUP_ADDRESSES_PER_TRANSACTION;
use crate::SolanaRpcClient;

/// Add extensions which make it possible to partially sign a versioned
/// transaction.
pub trait VersionedTransactionExtension {
	/// Create a new signed transaction from from the provided
	/// [`VersionedMessage`].
	fn new<T: Signers + ?Sized>(message: VersionedMessage, keypairs: &T) -> Self;
	/// Create a new unsigned transction from the payer and instructions with a
	/// recent blockhash. Under the hood this creates the message which needs to
	/// be signed.
	fn new_unsigned_v0(
		payer: &Pubkey,
		instructions: &[Instruction],
		address_lookup_tables: &[AddressLookupTableAccount],
		recent_blockhash: Hash,
	) -> Result<VersionedTransaction, CompileError>;
	fn new_unsigned(message: VersionedMessage) -> VersionedTransaction;
	/// Attempt to sign this transaction with provided signers.
	fn try_sign<T: Signers + ?Sized>(
		&mut self,
		signers: &T,
		recent_blockhash: Option<Hash>,
	) -> Result<&mut Self, SignerError>;
	fn try_sign_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<&mut Self>>;
	/// Sign the transaction with a subset of required keys, returning any
	/// errors.
	///
	/// This places each of the signatures created from `keypairs` in the
	/// corresponding position, as specified in the `positions` vector, in the
	/// transactions [`signatures`] field. It does not verify that the signature
	/// positions are correct.
	///
	/// [`signatures`]: VersionedTransaction::signatures
	///
	/// # Errors
	///
	/// Returns an error if signing fails.
	fn try_sign_unchecked<T: Signers + ?Sized>(
		&mut self,
		signers: &T,
		positions: Vec<usize>,
		recent_blockhash: Option<Hash>,
	) -> Result<(), SignerError>;
	fn try_sign_unchecked_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		position: usize,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<()>>;
	fn get_signing_keypair_positions(
		&self,
		pubkeys: &[Pubkey],
	) -> Result<Vec<Option<usize>>, SignerError>;
	/// Check whether the transaction is fully signed with valid signatures.
	fn is_signed(&self) -> bool;
	/// Sign the transaction with a subset of required keys, panicking when an
	/// error is met.
	fn sign<T: Signers + ?Sized>(
		&mut self,
		signers: &T,
		recent_blockhash: Option<Hash>,
	) -> &mut Self {
		self.try_sign(signers, recent_blockhash).unwrap()
	}
	/// Sign the transaction with a solana wallet.
	fn sign_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<&mut Self>>;
	fn sign_with_wallet<W: WalletSolanaSignTransaction>(
		self,
		wallet: &W,
		options: Option<SolanaSignTransactionOptions>,
	) -> impl Future<Output = WalletResult<VersionedTransaction>>;
}

impl VersionedTransactionExtension for VersionedTransaction {
	fn new<T: Signers + ?Sized>(message: VersionedMessage, keypairs: &T) -> Self {
		Self::try_new(message, keypairs).unwrap()
	}

	fn new_unsigned_v0(
		payer: &Pubkey,
		instructions: &[Instruction],
		address_lookup_tables: &[AddressLookupTableAccount],
		recent_blockhash: Hash,
	) -> Result<Self, CompileError> {
		let message =
			v0::Message::try_compile(payer, instructions, address_lookup_tables, recent_blockhash)?;
		let versioned_message = VersionedMessage::V0(message);

		Ok(Self::new_unsigned(versioned_message))
	}

	/// Create an unsigned transction from a [`VersionedMessage`].
	fn new_unsigned(message: VersionedMessage) -> Self {
		let signatures =
			vec![Signature::default(); message.header().num_required_signatures as usize];

		Self {
			signatures,
			message,
		}
	}

	/// Sign the transaction with a subset of required keys, returning any
	/// errors.
	///
	/// Unlike [`VersionedTransaction::try_new`], this method does not require
	/// all keypairs to be provided, allowing a transaction to be signed in
	/// multiple steps.
	///
	/// It is permitted to sign a transaction with the same keypair multiple
	/// times.
	///
	/// If `recent_blockhash` is different than recorded in the transaction
	/// message's [`VersionedMessage::recent_blockhash()`] method, then the
	/// message's `recent_blockhash` will be updated to the provided
	/// `recent_blockhash`, and any prior signatures will be cleared.
	fn try_sign<T: Signers + ?Sized>(
		&mut self,
		keypairs: &T,
		recent_blockhash: Option<Hash>,
	) -> Result<&mut Self, SignerError> {
		let positions = self
			.get_signing_keypair_positions(&keypairs.pubkeys())?
			.iter()
			.map(|pos| pos.ok_or(SignerError::KeypairPubkeyMismatch))
			.collect::<Result<Vec<_>, _>>()?;
		self.try_sign_unchecked(keypairs, positions, recent_blockhash)?;

		Ok(self)
	}

	fn try_sign_unchecked<T: Signers + ?Sized>(
		&mut self,
		keypairs: &T,
		positions: Vec<usize>,
		recent_blockhash: Option<Hash>,
	) -> Result<(), SignerError> {
		let message_blockhash = *self.message.recent_blockhash();
		let recent_blockhash = recent_blockhash.unwrap_or(message_blockhash);

		if recent_blockhash != message_blockhash {
			self.message.set_recent_blockhash(recent_blockhash);

			// reset signatures if blockhash has changed
			self.signatures
				.iter_mut()
				.for_each(|signature| *signature = Signature::default());
		}

		let signatures = keypairs.try_sign_message(&self.message.serialize())?;

		for ii in 0..positions.len() {
			self.signatures[positions[ii]] = signatures[ii];
		}

		Ok(())
	}

	/// Get the positions of the pubkeys in
	/// [`VersionedMessage::static_account_keys`] associated with
	/// signing keypairs.
	fn get_signing_keypair_positions(
		&self,
		pubkeys: &[Pubkey],
	) -> Result<Vec<Option<usize>>, SignerError> {
		let static_account_keys = self.message.static_account_keys();

		if static_account_keys.len() < self.message.header().num_required_signatures as usize {
			return Err(SignerError::InvalidInput("invalid message".to_string()));
		}

		let signed_keys =
			&static_account_keys[0..self.message.header().num_required_signatures as usize];

		Ok(pubkeys
			.iter()
			.map(|pubkey| signed_keys.iter().position(|x| x == pubkey))
			.collect())
	}

	fn is_signed(&self) -> bool {
		self.signatures.len() == self.message.header().num_required_signatures as usize
			&& self
				.signatures
				.iter()
				.all(|signature| *signature != Signature::default())
	}

	fn try_sign_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<&mut Self>> {
		async move {
			let Some(position) = self
				.get_signing_keypair_positions(&[wallet.solana_pubkey()])?
				.first()
				.copied()
				.flatten()
			else {
				return Err(SignerError::KeypairPubkeyMismatch.into());
			};
			self.try_sign_unchecked_async(wallet, position, recent_blockhash)
				.await?;

			Ok(self)
		}
	}

	fn try_sign_unchecked_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		position: usize,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<()>> {
		async move {
			let message_blockhash = *self.message.recent_blockhash();
			let recent_blockhash = recent_blockhash.unwrap_or(message_blockhash);

			if recent_blockhash != message_blockhash {
				self.message.set_recent_blockhash(recent_blockhash);

				// reset signatures if blockhash has changed
				self.signatures
					.iter_mut()
					.for_each(|signature| *signature = Signature::default());
			}

			let signature = wallet.sign_message_async(self.message.serialize()).await?;
			self.signatures[position] = signature.try_signature()?;

			Ok(())
		}
	}

	fn sign_async<W: WalletSolanaSignMessage + WalletSolanaPubkey>(
		&mut self,
		wallet: &W,
		recent_blockhash: Option<Hash>,
	) -> impl Future<Output = WalletResult<&mut Self>> {
		self.try_sign_async(wallet, recent_blockhash)
	}

	fn sign_with_wallet<W: WalletSolanaSignTransaction>(
		self,
		wallet: &W,
		options: Option<SolanaSignTransactionOptions>,
	) -> impl Future<Output = WalletResult<Self>> {
		async move {
			let transaction = wallet
				.sign_transaction(
					SolanaSignTransactionProps::builder()
						.transaction(self)
						.options_opt(options)
						.build(),
				)
				.await?
				.signed_transaction()?;

			Ok(transaction)
		}
	}
}

pub trait VersionedMessageExtension {
	fn into_versioned_transaction(self) -> VersionedTransaction;
}

impl VersionedMessageExtension for VersionedMessage {
	fn into_versioned_transaction(self) -> VersionedTransaction {
		VersionedTransaction::new_unsigned(self)
	}
}

/// Initialize a lookup table that can be used with versioned transactions.
pub async fn initialize_address_lookup_table<
	P: WalletSolanaSignTransaction + WalletSolanaPubkey,
	A: WalletSolanaSignMessage + WalletSolanaPubkey,
>(
	rpc: &SolanaRpcClient,
	payer_wallet: &P,
	authority_signer: &A,
	addresses: &[Pubkey],
) -> ClientResult<Pubkey> {
	let payer = payer_wallet.try_solana_pubkey()?;
	let authority = authority_signer.try_solana_pubkey()?;

	if addresses.len() > 256 {
		return Err(ClientError::Other(
			"Too many addresses passed to to the VersionedTransaction".into(),
		));
	}

	let mut address_chunks = addresses.chunks(MAX_LOOKUP_ADDRESSES_PER_TRANSACTION);
	let chunk = address_chunks.next().unwrap_or(&[]);
	let slot = rpc.get_slot().await? - 1;
	let (lookup_table_instruction, lookup_table_address) =
		create_lookup_table(payer, authority, slot);
	let mut instructions = vec![lookup_table_instruction];

	if !chunk.is_empty() {
		let instruction =
			extend_lookup_table(lookup_table_address, authority, Some(payer), chunk.into());

		instructions.push(instruction);
	}

	let mut versioned_transaction = VersionedTransaction::new_unsigned_v0(
		&payer,
		&instructions,
		&[],
		rpc.get_latest_blockhash().await?,
	)?
	.sign_with_wallet(payer_wallet, None)
	.await?;

	if payer != authority && !chunk.is_empty() {
		versioned_transaction
			.sign_async(authority_signer, None)
			.await?;
	}

	rpc.send_transaction(&versioned_transaction).await?;
	rpc.wait_for_new_block(1).await?;

	let instructions = address_chunks
		.map(|chunk| {
			extend_lookup_table(lookup_table_address, authority, Some(payer), chunk.into())
		})
		.collect::<Vec<_>>();

	let Some(instruction) = instructions.first().map(wasm_safe_instruction_clone) else {
		return Ok(lookup_table_address);
	};

	let compute_limit_instruction =
		ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNIT_MAX_LIMIT as u32);
	let versioned_transaction = VersionedTransaction::new_unsigned_v0(
		&payer,
		&[compute_limit_instruction, instruction],
		&[],
		rpc.get_latest_blockhash().await?,
	)?;
	let Some(compute_units) = rpc
		.simulate_transaction(&versioned_transaction)
		.await?
		.value
		.units_consumed
		.map(|c| c.mul(110).div(100))
	else {
		return Err(ClientError::Other(
			"Could not calculate the optimal compute units".into(),
		));
	};
	let chunk_size = COMPUTE_UNIT_MAX_LIMIT.div(compute_units as usize);
	let instruction_chunks = instructions.chunks(chunk_size);

	for instruction_chunk in instruction_chunks {
		let compute_limit_instruction = ComputeBudgetInstruction::set_compute_unit_limit(
			instruction_chunk.len().mul(compute_units as usize) as u32,
		);
		let mut instructions = vec![compute_limit_instruction];
		let mut cloned_chunk = instruction_chunk
			.iter()
			.map(wasm_safe_instruction_clone)
			.collect::<Vec<_>>();
		instructions.append(&mut cloned_chunk);
		let mut versioned_transaction = VersionedTransaction::new_unsigned_v0(
			&payer,
			&instructions,
			&[],
			rpc.get_latest_blockhash().await?,
		)?
		.sign_with_wallet(payer_wallet, None)
		.await?;

		if payer != authority {
			versioned_transaction
				.sign_async(authority_signer, None)
				.await?;
		}

		rpc.send_transaction(&versioned_transaction).await?;
	}

	Ok(lookup_table_address)
}

fn wasm_safe_instruction_clone(instruction: &Instruction) -> Instruction {
	#[cfg(target_arch = "wasm32")]
	{
		Instruction {
			program_id: instruction.program_id.clone(),
			accounts: instruction.accounts.clone(),
			data: instruction.data.clone(),
		}
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		instruction.clone()
	}
}
