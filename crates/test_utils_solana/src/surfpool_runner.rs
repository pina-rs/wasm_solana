//! An in-process surfpool node, the fast counterpart of the agave
//! [`TestValidatorRunner`](crate::TestValidatorRunner).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use solana_account::AccountSharedData;
use solana_account::ReadableAccount;
use solana_commitment_config::CommitmentConfig;
use solana_commitment_config::CommitmentLevel;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::Pubkey;
use surfpool_sdk::BlockProductionMode;
use surfpool_sdk::Pubkey as SurfpoolPubkey;
use surfpool_sdk::Surfnet;
use surfpool_sdk::cheatcodes::builders::DeployProgram;
use typed_builder::TypedBuilder;
use wasm_client_solana::SolanaRpcClient;

/// Description of a program to deploy into a [`SurfpoolRunner`].
///
/// Unlike [`TestProgramInfo`](crate::TestProgramInfo) there is no upgrade
/// authority or loader to pick: surfpool writes the program bytes directly
/// through its `surfnet_writeProgram` cheatcode.
#[derive(Debug, Clone, TypedBuilder)]
pub struct SurfpoolProgramInfo {
	/// The address the program is deployed at.
	pub program_id: Pubkey,
	/// Path to the compiled program `.so` file.
	#[builder(setter(into))]
	pub program_path: PathBuf,
}

/// Configuration for a [`SurfpoolRunner`], built with its
/// [`TypedBuilder`](typed_builder::TypedBuilder) implementation.
///
/// The surface mirrors
/// [`TestValidatorRunnerProps`](crate::TestValidatorRunnerProps) where the two
/// backends agree. Ports are always dynamic (surfpool picks free ports),
/// `warp_slot` and `epoch_schedule` have no equivalent — surfpool slots advance
/// per transaction by default, so there is no genesis warm-up to skip — and
/// time travel is available at runtime through [`SurfpoolRunner::surfnet`]
/// instead.
#[derive(Debug, Clone, TypedBuilder)]
pub struct SurfpoolRunnerProps {
	/// The pubkeys to fund with `initial_lamports` at startup. The
	/// [`SurfpoolRunner::payer`] is funded with the same amount in addition
	/// to these.
	#[builder(default)]
	pub pubkeys: Vec<Pubkey>,
	/// The lamports funded into each of `pubkeys` and the payer.
	///
	/// The default amount is `5.0 SOL`.
	#[builder(default = sol_str_to_lamports("5.0").unwrap())]
	pub initial_lamports: u64,
	/// Programs to deploy before the runner is returned.
	#[builder(default)]
	pub programs: Vec<SurfpoolProgramInfo>,
	/// Custom accounts to seed before the runner is returned, applied with
	/// surfpool's `surfnet_setAccount` cheatcode.
	#[builder(default)]
	pub accounts: HashMap<Pubkey, AccountSharedData>,
	/// The default commitment level for the runner's rpc client.
	///
	/// Defaults to `Confirmed`: surfpool's per-transaction block mode — the
	/// runner's default — advances transactions to confirmed without
	/// finalizing roots, so a `Finalized` client would wait out every
	/// confirmation budget. Tests that genuinely need finality should switch
	/// `block_production_mode` to `Clock` (or `Manual`) so roots advance.
	#[builder(default = CommitmentLevel::Confirmed, setter(into))]
	pub commitment: CommitmentLevel,
	/// How blocks are produced. Defaults to
	/// [`BlockProductionMode::Transaction`], advancing a slot per
	/// transaction — the fastest mode, but one that never finalizes roots;
	/// pick `Clock` for time-driven slots (with [`Self::slot_time_ms`])
	/// when a test needs finality.
	#[builder(default = BlockProductionMode::Transaction, setter(into))]
	pub block_production_mode: BlockProductionMode,
	/// Slot time in milliseconds, used by `Clock` block production; the
	/// default of one millisecond keeps blockhashes and subscriptions moving
	/// far faster than real time.
	#[builder(default = 1)]
	pub slot_time_ms: u64,
	/// Skip blockhash validation for every transaction sent to this
	/// surfnet, so tests never race an expired blockhash.
	#[builder(default)]
	pub skip_blockhash_check: bool,
	/// An optional remote RPC endpoint (for example a mainnet one) that the
	/// surfnet lazily fetches accounts and programs from. Setting it implies
	/// online mode; leaving it `None` keeps the surfnet fully offline.
	#[builder(default, setter(strip_option))]
	pub remote_rpc_url: Option<String>,
}

impl SurfpoolRunnerProps {
	/// Start a [`SurfpoolRunner`] with these props.
	///
	/// ```rust
	/// use solana_pubkey::pubkey;
	/// use test_utils_solana::SurfpoolRunnerProps;
	///
	/// # async fn run() {
	/// let user = pubkey!("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");
	/// let runner = SurfpoolRunnerProps::builder()
	/// 	.pubkeys(vec![user])
	/// 	.build()
	/// 	.run()
	/// 	.await;
	/// # }
	/// ```
	pub async fn run(self) -> SurfpoolRunner {
		SurfpoolRunner::run(self).await
	}
}

/// A locally running surfpool node, embedded in-process by `surfpool-sdk`.
///
/// Where [`TestValidatorRunner`](crate::TestValidatorRunner) boots a full
/// agave validator — genesis, a RocksDB ledger in a temporary directory,
/// and a faucet thread, seconds before the first RPC answers — a
/// `SurfpoolRunner` starts a [`Surfnet`] simulation in milliseconds: slots
/// advance per transaction by default, `requestAirdrop` works out of the
/// box, and
/// cheatcodes (see [`Surfnet::cheatcodes`]) mutate balances, accounts and
/// time directly. Tests that depend on validator internals — vote accounts,
/// rewards, the completeness of `getBlock` — should keep using the agave
/// runner; this one is for fast iteration on client behavior.
///
/// Ports are dynamic; use [`Self::rpc_url`] and [`Self::pubsub_url`]
/// instead of assuming surfpool's CLI defaults (8899/8900). Dropping every
/// clone of the runner stops the surfnet.
#[derive(Clone)]
pub struct SurfpoolRunner {
	surfnet: Arc<Surfnet>,
	/// The rpc client for the surfnet.
	rpc: SolanaRpcClient,
}

impl SurfpoolRunner {
	async fn run_internal(
		SurfpoolRunnerProps {
			pubkeys,
			initial_lamports,
			programs,
			accounts,
			commitment,
			block_production_mode,
			slot_time_ms,
			skip_blockhash_check,
			remote_rpc_url,
		}: SurfpoolRunnerProps,
	) -> Result<Self> {
		let mut builder = Surfnet::builder()
			.airdrop_addresses(pubkeys.iter().map(to_surfpool_pubkey).collect())
			.airdrop_sol(initial_lamports)
			.block_production_mode(block_production_mode)
			.slot_time_ms(slot_time_ms)
			.skip_blockhash_check(skip_blockhash_check);

		if let Some(url) = remote_rpc_url {
			builder = builder.remote_rpc_url(url);
		}

		let surfnet = builder.start().await?;
		let cheatcodes = surfnet.cheatcodes();

		for (pubkey, account) in accounts {
			cheatcodes.set_account(
				&to_surfpool_pubkey(&pubkey),
				account.lamports(),
				account.data(),
				&to_surfpool_pubkey(account.owner()),
			)?;
		}

		for program in programs {
			cheatcodes.deploy(
				DeployProgram::new(to_surfpool_pubkey(&program.program_id))
					.so_path(program.program_path),
			)?;
		}

		let rpc = SolanaRpcClient::new_with_ws_and_commitment(
			surfnet.rpc_url(),
			surfnet.ws_url(),
			CommitmentConfig { commitment },
		);

		Ok(Self {
			surfnet: Arc::new(surfnet),
			rpc,
		})
	}

	/// Create a new runner for an in-process surfpool node.
	///
	/// ```rust
	/// use test_utils_solana::SurfpoolRunner;
	/// use test_utils_solana::SurfpoolRunnerProps;
	///
	/// # async fn run() -> SurfpoolRunner {
	/// SurfpoolRunner::run(SurfpoolRunnerProps::builder().build()).await
	/// # }
	/// ```
	pub async fn run(props: SurfpoolRunnerProps) -> Self {
		Self::run_internal(props).await.unwrap()
	}

	/// URL of the surfnet's JSON RPC endpoint.
	pub fn rpc_url(&self) -> String {
		self.surfnet.rpc_url().to_owned()
	}

	/// URL of the surfnet's pubsub (websocket) endpoint. Surfpool picks a
	/// dynamic port that is *not* `rpc_url`'s port plus one, so always use
	/// this instead of deriving it.
	pub fn pubsub_url(&self) -> String {
		self.surfnet.ws_url().to_owned()
	}

	/// The RPC client connected to this surfnet.
	pub fn rpc(&self) -> &SolanaRpcClient {
		&self.rpc
	}

	/// The surfnet's pre-funded payer keypair, funded with the runner's
	/// `initial_lamports`.
	pub fn payer(&self) -> &Keypair {
		self.surfnet.payer()
	}

	/// The underlying [`Surfnet`], for cheatcodes, events and commands.
	pub fn surfnet(&self) -> &Surfnet {
		&self.surfnet
	}
}

/// Convert this workspace's `solana-pubkey` into surfpool's.
///
/// Surfpool pins its own agave crate graph, whose `solana-pubkey` is a
/// different major than the one this crate uses, so the boundary only ever
/// crosses as base58.
fn to_surfpool_pubkey(pubkey: &Pubkey) -> SurfpoolPubkey {
	pubkey
		.to_string()
		.parse()
		.expect("a base58 pubkey parses on both sides of the version split")
}
