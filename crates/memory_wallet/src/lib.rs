#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/readme.md"))]

use std::hash::Hash;

use async_trait::async_trait;
use derive_more::derive::Deref;
use derive_more::derive::DerefMut;
use futures::future::try_join_all;
use indexmap::Equivalent;
use indexmap::IndexSet;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use wallet_standard::SOLANA_SIGN_AND_SEND_TRANSACTION;
use wallet_standard::SOLANA_SIGN_IN;
use wallet_standard::SOLANA_SIGN_MESSAGE;
use wallet_standard::SOLANA_SIGN_TRANSACTION;
use wallet_standard::STANDARD_CONNECT;
use wallet_standard::STANDARD_DISCONNECT;
use wallet_standard::STANDARD_EVENTS;
use wallet_standard::SolanaSignAndSendTransactionProps;
use wallet_standard::SolanaSignInInput;
use wallet_standard::SolanaSignTransactionProps;
use wallet_standard::StandardConnectInput;
use wallet_standard::create_sign_in_message_text;
use wallet_standard::prelude::*;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::prelude::*;

#[derive(Debug, Deref, DerefMut)]
pub struct MemoryWalletAccountInfo {
	#[deref]
	#[deref_mut]
	keypair: Keypair,
	label: Option<String>,
	icon: Option<String>,
}

impl Default for MemoryWalletAccountInfo {
	fn default() -> Self {
		Self {
			keypair: Keypair::new(),
			label: None,
			icon: None,
		}
	}
}

impl PartialEq for MemoryWalletAccountInfo {
	fn eq(&self, other: &Self) -> bool {
		self.keypair.eq(&other.keypair)
	}
}

impl Equivalent<MemoryWalletAccountInfo> for Pubkey {
	fn equivalent(&self, key: &MemoryWalletAccountInfo) -> bool {
		self.eq(&key.keypair.pubkey())
	}
}

impl Eq for MemoryWalletAccountInfo {}

impl Hash for MemoryWalletAccountInfo {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		let pubkey = &self.keypair.pubkey();
		pubkey.hash(state);
	}
}

impl Clone for MemoryWalletAccountInfo {
	fn clone(&self) -> Self {
		Self {
			keypair: self.keypair.insecure_clone(),
			label: self.label.clone(),
			icon: self.icon.clone(),
		}
	}
}

impl MemoryWalletAccountInfo {
	pub fn new() -> Self {
		Self {
			keypair: Keypair::new(),
			label: None,
			icon: None,
		}
	}

	pub fn new_labelled(label: String) -> Self {
		Self {
			keypair: Keypair::new(),
			label: Some(label),
			icon: None,
		}
	}
}

impl From<&Keypair> for MemoryWalletAccountInfo {
	fn from(value: &Keypair) -> Self {
		Self {
			keypair: value.insecure_clone(),
			label: None,
			icon: None,
		}
	}
}

impl WalletAccountInfo for MemoryWalletAccountInfo {
	fn address(&self) -> String {
		Signer::pubkey(&self.keypair).to_string()
	}

	fn public_key(&self) -> Vec<u8> {
		Signer::pubkey(&self.keypair).to_bytes().to_vec()
	}

	fn chains(&self) -> Vec<String> {
		vec!["solana".into()]
	}

	fn features(&self) -> Vec<String> {
		MEMORY_WALLET_FEATURES.map(Into::into).to_vec()
	}

	fn label(&self) -> Option<String> {
		self.label.clone()
	}

	fn icon(&self) -> Option<String> {
		self.icon.clone()
	}
}

#[derive(Clone, Debug)]
pub struct MemoryWalletInfo {
	accounts: IndexSet<MemoryWalletAccountInfo>,
}

impl WalletInfo for MemoryWalletInfo {
	type Account = MemoryWalletAccountInfo;

	fn version(&self) -> String {
		"1.0.0".into()
	}

	fn name(&self) -> String {
		"Solana Memory Wallet".into()
	}

	fn icon(&self) -> String {
		String::new()
	}

	fn chains(&self) -> Vec<String> {
		vec!["solana".into()]
	}

	fn features(&self) -> Vec<String> {
		MEMORY_WALLET_FEATURES.map(Into::into).to_vec()
	}

	fn accounts(&self) -> Vec<Self::Account> {
		self.accounts.clone().into_iter().collect()
	}
}

#[derive(Clone, Debug)]
pub struct MemoryWallet {
	wallet: MemoryWalletInfo,
	account: Option<MemoryWalletAccountInfo>,
	rpc: SolanaRpcClient,
}

impl Signer for MemoryWallet {
	fn try_pubkey(&self) -> Result<Pubkey, solana_signer::SignerError> {
		let Some(ref account) = self.account else {
			return Err(solana_signer::SignerError::Connection(
				"No connected account".into(),
			));
		};

		account.try_pubkey()
	}

	fn try_sign_message(&self, message: &[u8]) -> Result<Signature, solana_signer::SignerError> {
		let Some(ref account) = self.account else {
			return Err(solana_signer::SignerError::Connection(
				"No connected account".into(),
			));
		};

		account.try_sign_message(message)
	}

	fn is_interactive(&self) -> bool {
		true
	}
}

impl MemoryWallet {
	pub fn new(rpc: SolanaRpcClient, accounts: &[Keypair]) -> Self {
		let accounts = accounts
			.iter()
			.map(Into::into)
			.collect::<IndexSet<MemoryWalletAccountInfo>>();
		let account = accounts.first().cloned();
		let wallet = MemoryWalletInfo { accounts };

		Self {
			wallet,
			account,
			rpc,
		}
	}

	pub fn add_primary_account(
		&mut self,
		account: impl Into<MemoryWalletAccountInfo>,
	) -> &mut Self {
		let account = account.into();

		self.wallet.accounts.shift_insert(0, account.clone());
		self.account = Some(account);

		self
	}

	/// Set the primary account to the account matching the provided pubkey.
	pub fn set_primary_account(&mut self, pubkey: &Pubkey) -> &mut Self {
		let account = self.wallet.accounts.get(pubkey);

		self.account = account.cloned();

		self
	}
}

impl Wallet for MemoryWallet {
	type Account = MemoryWalletAccountInfo;
	type Wallet = MemoryWalletInfo;

	fn wallet(&self) -> Self::Wallet {
		self.wallet.clone()
	}

	fn wallet_account(&self) -> Option<Self::Account> {
		self.account.clone()
	}
}

#[async_trait(?Send)]
impl WalletStandardConnect for MemoryWallet {
	async fn connect(&mut self) -> WalletResult<Vec<Self::Account>> {
		let Some(account) = self.wallet.accounts.first() else {
			return Err(WalletError::WalletConnection);
		};

		self.account = Some(account.clone());

		Ok(self.wallet.accounts())
	}

	async fn connect_with_options(
		&mut self,
		_: StandardConnectInput,
	) -> WalletResult<Vec<Self::Account>> {
		self.connect().await
	}
}

#[async_trait(?Send)]
impl WalletStandardDisconnect for MemoryWallet {
	async fn disconnect(&mut self) -> WalletResult<()> {
		self.account = None;

		Ok(())
	}
}

#[async_trait(?Send)]
impl WalletSolanaSignAndSendTransaction for MemoryWallet {
	type Output = Signature;

	async fn sign_and_send_transaction(
		&self,
		SolanaSignAndSendTransactionProps {
			mut transaction, ..
		}: SolanaSignAndSendTransactionProps,
	) -> WalletResult<Self::Output> {
		let Some(ref account) = self.account else {
			return Err(WalletError::WalletNotConnected);
		};

		let message_blockhash = *transaction.message.recent_blockhash();

		transaction.try_sign(
			&[&**account],
			if message_blockhash == solana_hash::Hash::default() {
				Some(self.rpc.get_latest_blockhash().await?)
			} else {
				None
			},
		)?;
		let signature = self.rpc.send_transaction(&transaction).await?;

		Ok(signature)
	}

	async fn sign_and_send_transactions(
		&self,
		inputs: Vec<SolanaSignAndSendTransactionProps>,
	) -> WalletResult<Vec<Self::Output>> {
		let futures = inputs
			.into_iter()
			.map(|input| self.sign_and_send_transaction(input));

		try_join_all(futures).await
	}
}

#[async_trait(?Send)]
impl WalletSolanaSignTransaction for MemoryWallet {
	type Output = VersionedTransaction;

	async fn sign_transaction(
		&self,
		SolanaSignTransactionProps {
			mut transaction, ..
		}: SolanaSignTransactionProps,
	) -> WalletResult<Self::Output> {
		let Some(ref account) = self.account else {
			return Err(WalletError::WalletNotConnected);
		};

		let message_blockhash = *transaction.message.recent_blockhash();

		transaction.try_sign(
			&[&**account],
			if message_blockhash == solana_hash::Hash::default() {
				Some(self.rpc.get_latest_blockhash().await?)
			} else {
				None
			},
		)?;

		Ok(transaction)
	}

	async fn sign_transactions(
		&self,
		inputs: Vec<SolanaSignTransactionProps>,
	) -> WalletResult<Vec<Self::Output>> {
		let futures = inputs.into_iter().map(|input| self.sign_transaction(input));

		try_join_all(futures).await
	}
}

#[derive(Clone, Debug)]
pub struct MemorySolanaSignInOutput {
	signature: Signature,
	account: MemoryWalletAccountInfo,
	signed_message: Vec<u8>,
}

impl SolanaSignatureOutput for MemorySolanaSignInOutput {
	fn try_signature(&self) -> WalletResult<Signature> {
		Ok(self.signature)
	}

	fn signature(&self) -> Signature {
		self.signature
	}
}

impl SolanaSignMessageOutput for MemorySolanaSignInOutput {
	fn signed_message(&self) -> Vec<u8> {
		self.signed_message.clone()
	}

	fn signature_type(&self) -> Option<String> {
		Some("Ed25519".into())
	}
}

impl SolanaSignInOutput for MemorySolanaSignInOutput {
	type Account = MemoryWalletAccountInfo;

	fn account(&self) -> Self::Account {
		self.account.clone()
	}
}

#[async_trait(?Send)]
impl WalletSolanaSignIn for MemoryWallet {
	type Output = MemorySolanaSignInOutput;

	async fn sign_in(&self, input: SolanaSignInInput) -> WalletResult<Self::Output> {
		let Some(ref account) = self.account else {
			return Err(WalletError::WalletNotConnected);
		};

		let sign_in_message = create_sign_in_message_text(&input)?.into_bytes();
		let signed_message = nacl::sign::sign(&sign_in_message, &account.keypair.to_bytes())
			.map_err(|e| WalletError::Signer(format!("{e:?}")))?;
		let signature = account.keypair.try_sign_message(&sign_in_message)?;

		Ok(MemorySolanaSignInOutput {
			signature,
			account: account.clone(),
			signed_message,
		})
	}

	async fn sign_in_many(
		&self,
		inputs: Vec<SolanaSignInInput>,
	) -> WalletResult<Vec<Self::Output>> {
		let futures = inputs.into_iter().map(|input| self.sign_in(input));

		try_join_all(futures).await
	}
}

pub struct MemorySolanaSignMessageOutput {
	signature: Signature,
	signed_message: Vec<u8>,
}

impl SolanaSignatureOutput for MemorySolanaSignMessageOutput {
	fn try_signature(&self) -> WalletResult<Signature> {
		Ok(self.signature)
	}

	fn signature(&self) -> Signature {
		self.signature
	}
}

impl SolanaSignMessageOutput for MemorySolanaSignMessageOutput {
	fn signed_message(&self) -> Vec<u8> {
		self.signed_message.clone()
	}

	fn signature_type(&self) -> Option<String> {
		Some("Ed25519".into())
	}
}

#[async_trait(?Send)]
impl WalletSolanaSignMessage for MemoryWallet {
	type Output = MemorySolanaSignMessageOutput;

	/// Sign a  message using the account's secret key.
	async fn sign_message_async(&self, message: impl Into<Vec<u8>>) -> WalletResult<Self::Output> {
		let Some(ref account) = self.account else {
			return Err(WalletError::WalletNotConnected);
		};

		let message = message.into();
		let signed_message = nacl::sign::sign(&message, &account.keypair.to_bytes())
			.map_err(|e| WalletError::Signer(format!("{e:?}")))?;
		let signature = Signer::try_sign_message(&account.keypair, &message)?;

		Ok(MemorySolanaSignMessageOutput {
			signature,
			signed_message,
		})
	}

	/// Sign a list of messages using the account's secret key.
	async fn sign_messages<M: Into<Vec<u8>>>(
		&self,
		messages: Vec<M>,
	) -> WalletResult<Vec<Self::Output>> {
		let futures = messages
			.into_iter()
			.map(|message| WalletSolanaSignMessage::sign_message_async(self, message));

		try_join_all(futures).await
	}
}

pub const MEMORY_WALLET_FEATURES: [&str; 7] = [
	STANDARD_CONNECT,
	STANDARD_DISCONNECT,
	STANDARD_EVENTS,
	SOLANA_SIGN_MESSAGE,
	SOLANA_SIGN_IN,
	SOLANA_SIGN_TRANSACTION,
	SOLANA_SIGN_AND_SEND_TRANSACTION,
];

pub mod prelude {
	pub use wallet_standard::prelude::*;
	pub use wasm_client_solana::prelude::*;
}
