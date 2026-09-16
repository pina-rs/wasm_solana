use std::collections::HashMap;
use std::collections::HashSet;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;

use anyhow::Result;
use crossbeam_channel::unbounded;
use port_check::is_local_ipv4_port_free;
use rand::Rng;
use solana_account::AccountSharedData;
use solana_clock::Slot;
use solana_commitment_config::CommitmentConfig;
use solana_commitment_config::CommitmentLevel;
use solana_epoch_schedule::EpochSchedule;
use solana_faucet::faucet::LocalFaucetConfig;
use solana_faucet::faucet::run_local_faucet_with_config;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::Pubkey;
use solana_rpc::rpc::JsonRpcConfig;
use solana_signer::Signer;
use solana_system_interface::program as system_program;
use solana_test_validator::TestValidator;
pub use solana_test_validator::TestValidatorGenesis;
use solana_test_validator::UpgradeableProgramInfo;
use tempfile::TempDir;
use tempfile::tempdir;
use typed_builder::TypedBuilder;
use wasm_client_solana::SolanaRpcClient;

/// Configuration for a [`TestValidatorRunner`], built with its
/// [`TypedBuilder`](typed_builder::TypedBuilder) implementation.
#[derive(Debug, Clone, TypedBuilder)]
pub struct TestValidatorRunnerProps {
	/// The ports to use for this runner. Defaults to all three ports being
	/// random. Can be overriden.
	#[builder(default = TestValidatorPorts::random_ports())]
	pub ports: TestValidatorPorts,
	/// The programs to add to the validator.
	#[builder(default)]
	pub programs: Vec<TestProgramInfo>,
	/// The funded pubkeys to fund with an amount of sol each. The amount can be
	/// overriden via [`TestValidatorRunnerProps::initial_lamports`]. For more
	/// custom control on funded accounts you can use the `accounts` field.
	#[builder(default)]
	pub pubkeys: Vec<Pubkey>,
	/// The initial lamports to add to the defined
	/// [`TestValidatorRunnerProps::pubkeys`].
	///
	/// The default amount is `5.0 SOL`.
	#[builder(default = sol_str_to_lamports("5.0").unwrap())]
	pub initial_lamports: u64,
	/// The default commitment level to use for the validator client rpc.
	#[builder(default, setter(into))]
	pub commitment: CommitmentLevel,
	/// Custom accounts to add during genesis. These accounts can include custom
	/// data and state.
	#[builder(default)]
	pub accounts: HashMap<Pubkey, AccountSharedData>,
	/// Warp the ledger to `warp_slot` after starting the validator.
	#[builder(default = 1000, setter(into))]
	pub warp_slot: Slot,
	/// Override the epoch schedule.
	#[builder(default)]
	pub epoch_schedule: EpochSchedule,
}

impl Default for TestValidatorRunnerProps {
	fn default() -> Self {
		Self::builder().build()
	}
}

impl TestValidatorRunnerProps {
	/// Defers to the [`TestValidatorRunner::run`] method with the props
	/// defined in this struct.
	///
	/// ```rust
	/// use solana_native_token::sol_str_to_lamports;
	/// use solana_pubkey::pubkey;
	/// use test_utils_solana::TestValidatorRunnerProps;
	///
	/// async fn run() {
	/// 	let user = pubkey!("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin");
	/// 	let runner = TestValidatorRunnerProps::builder()
	/// 		.pubkeys(vec![user])
	/// 		.initial_lamports(sol_str_to_lamports("2.0").unwrap())
	/// 		.build()
	/// 		.run()
	/// 		.await;
	/// }
	/// ```
	pub async fn run(self) -> TestValidatorRunner {
		TestValidatorRunner::run(self).await
	}
}

/// Description of an upgradeable program to load into the [`TestValidator`].
#[derive(Debug, Clone, TypedBuilder)]
pub struct TestProgramInfo {
	/// The address the program is loaded at.
	pub program_id: Pubkey,
	/// Path to the compiled program `.so` file.
	#[builder(setter(into))]
	pub program_path: PathBuf,
	/// Authority allowed to upgrade the program. Defaults to no authority.
	#[builder(default = Pubkey::default())]
	pub upgrade_authority: Pubkey,
	/// The loader program that owns this program. Defaults to the upgradeable
	/// BPF loader.
	#[builder(default = solana_sdk_ids::bpf_loader_upgradeable::ID)]
	pub loader: Pubkey,
}

impl From<TestProgramInfo> for UpgradeableProgramInfo {
	fn from(
		TestProgramInfo {
			program_id,
			program_path,
			upgrade_authority,
			loader,
		}: TestProgramInfo,
	) -> Self {
		Self {
			program_id,
			loader,
			upgrade_authority,
			program_path,
		}
	}
}

/// The ports a [`TestValidatorRunner`] binds to.
#[derive(Debug, Copy, Clone, TypedBuilder)]
pub struct TestValidatorPorts {
	/// Port for the JSON RPC endpoint.
	#[builder(default = 8899)]
	pub rpc: u16,
	/// Port for the pubsub (websocket) endpoint.
	#[builder(default = 8900)]
	pub pubsub: u16,
	/// Port for the faucet endpoint used by airdrops.
	#[builder(default = 9900)]
	pub faucet: u16,
	/// Inclusive range of ports used for gossip.
	#[builder(default = (8001, 8021))]
	pub gossip_range: (u16, u16),
}

impl Default for TestValidatorPorts {
	fn default() -> Self {
		Self::builder().build()
	}
}

impl TestValidatorPorts {
	/// Find a free set of ports, returning `None` if none could be found within
	/// the retry budget.
	pub fn try_random_ports() -> Option<Self> {
		find_ports().map(|(rpc, pubsub, faucet, gossip_range)| {
			Self {
				rpc,
				pubsub,
				faucet,
				gossip_range,
			}
		})
	}

	/// Find a free set of ports, panicking if none could be found.
	pub fn random_ports() -> Self {
		Self::try_random_ports().unwrap()
	}
}

/// A local test validator runner which can be used for the test validator.
#[derive(Clone)]
pub struct TestValidatorRunner {
	genesis: Arc<TestValidatorGenesis>,
	/// The ports used for the validator.
	/// The first port is the `rpc_port`, the second is the `pubsub_port`, and
	/// the third is the `faucet_port` to allow for airdrops.
	ports: TestValidatorPorts,
	/// The original wrapped test validator
	validator: Arc<TestValidator>,
	/// This is the keypair for the mint account and is funded with 500 SOL.
	mint_keypair: Arc<Keypair>,
	/// The rpc client for the validator.
	rpc: SolanaRpcClient,
	/// This can be RAM intensive so use a tempdir when required.
	ledger_path: Arc<TempDir>,
}

impl TestValidatorRunner {
	async fn run_internal(
		TestValidatorRunnerProps {
			ports,
			programs,
			pubkeys,
			initial_lamports,
			commitment,
			accounts,
			warp_slot,
			epoch_schedule,
		}: TestValidatorRunnerProps,
	) -> Result<Self> {
		let mut genesis = TestValidatorGenesis::default();
		let faucet_keypair = Keypair::new();
		let faucet_pubkey = faucet_keypair.pubkey();
		let programs = programs.into_iter().map(Into::into).collect::<Vec<_>>();
		let ledger_path = Arc::new(tempdir()?);

		mark_port_used(ports.rpc);
		mark_port_used(ports.pubsub);
		mark_port_used(ports.faucet);

		for port in ports.gossip_range.0..=ports.gossip_range.1 {
			mark_port_used(port);
		}

		let (sender, receiver) = unbounded();
		let faucet_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), ports.faucet);
		// run the faucet in a separate thread
		run_local_faucet_with_config(
			sender,
			LocalFaucetConfig {
				keypair: faucet_keypair,
				address: Ipv4Addr::LOCALHOST,
				port: ports.faucet,
				time_input: None,
				per_time_cap: None,
				per_request_cap: None,
			},
		);

		let _ = receiver
			.recv()
			.expect("run solana faucet")
			.expect("there was an error running the solana faucet");

		let funded_accounts = pubkeys.iter().map(|pubkey| {
			(
				*pubkey,
				AccountSharedData::new(initial_lamports, 0, &Pubkey::default()),
			)
		});

		genesis
			.rpc_port(ports.rpc)
			.gossip_port(ports.gossip_range.0)
			.port_range(ports.gossip_range)
			.ledger_path(ledger_path.path())
			.rpc_config(JsonRpcConfig {
				faucet_addr: Some(faucet_addr),
				enable_rpc_transaction_history: true,
				..JsonRpcConfig::default_for_test()
			})
			// Needed to prevent all account transactions from failing with this error:
			// `Attempt to debit an account but found no record of a prior credit.`
			.warp_slot(warp_slot)
			.epoch_schedule(epoch_schedule)
			.add_upgradeable_programs_with_path(&programs)
			.add_account(
				faucet_pubkey,
				AccountSharedData::new(
					sol_str_to_lamports("1000000.0").unwrap(),
					0,
					&system_program::ID,
				),
			)
			.add_accounts(funded_accounts)
			.add_accounts(accounts);

		let (validator, mint_keypair) = genesis.start_async().await;

		let rpc = SolanaRpcClient::new_with_ws_and_commitment(
			&validator.rpc_url(),
			&validator.rpc_pubsub_url(),
			CommitmentConfig { commitment },
		);

		// waiting for fees to stablize doesn't seem to work, so here waiting
		// for this random airdrop to succeed seems to work. An alternative is
		// a 15 second daily. The validator to be warmed up.
		rpc.request_airdrop(
			&mint_keypair.pubkey(),
			sol_str_to_lamports("500.0").unwrap(),
		)
		.await?;

		let runner = Self {
			genesis: Arc::new(genesis),
			ports,
			validator: Arc::new(validator),
			mint_keypair: Arc::new(mint_keypair),
			rpc,
			ledger_path,
		};

		Ok(runner)
	}

	/// Create a new runner for the solana test validator.
	///
	/// ```rust
	/// use test_utils_solana::TestValidatorRunner;
	/// use test_utils_solana::TestValidatorRunnerProps;
	///
	/// async fn run() -> TestValidatorRunner {
	/// 	TestValidatorRunner::run(TestValidatorRunnerProps::default()).await
	/// }
	/// ```
	pub async fn run(props: TestValidatorRunnerProps) -> Self {
		Self::run_internal(props).await.unwrap()
	}

	/// URL of the validator's JSON RPC endpoint.
	pub fn rpc_url(&self) -> String {
		self.validator.rpc_url()
	}

	/// URL of the validator's pubsub (websocket) endpoint.
	pub fn pubsub_url(&self) -> String {
		self.validator.rpc_pubsub_url()
	}

	/// The RPC client connected to this validator.
	pub fn rpc(&self) -> &SolanaRpcClient {
		&self.rpc
	}

	/// The underlying [`TestValidator`] instance.
	pub fn validator(&self) -> &TestValidator {
		&self.validator
	}

	/// The genesis configuration used to start the validator.
	pub fn genesis(&self) -> &TestValidatorGenesis {
		&self.genesis
	}

	/// The ports the validator is listening on.
	pub fn ports(&self) -> TestValidatorPorts {
		self.ports
	}

	/// The validator's mint keypair, funded with 500 SOL.
	pub fn mint_keypair(&self) -> &Keypair {
		&self.mint_keypair
	}

	/// Path to the temporary ledger directory backing the validator.
	pub fn ledger_path(&self) -> PathBuf {
		self.ledger_path.path().to_owned()
	}
}

impl Drop for TestValidatorRunner {
	fn drop(&mut self) {
		free_port(self.ports.rpc);
		free_port(self.ports.pubsub);
		free_port(self.ports.faucet);

		for port in self.ports.gossip_range.0..=self.ports.gossip_range.1 {
			free_port(port);
		}
	}
}

static USED_PORTS: LazyLock<Arc<Mutex<HashSet<u16>>>> =
	LazyLock::new(|| Arc::new(Mutex::new(HashSet::new())));

fn is_port_available(port: u16) -> bool {
	let used_ports = USED_PORTS.lock().unwrap();

	is_local_ipv4_port_free(port) && !used_ports.contains(&port)
}

fn mark_port_used(port: u16) {
	let mut used_ports = USED_PORTS.lock().unwrap();
	used_ports.insert(port);
}

fn free_port(port: u16) {
	let mut used_ports = USED_PORTS.lock().unwrap();
	used_ports.remove(&port);
}

fn find_ports() -> Option<(u16, u16, u16, (u16, u16))> {
	let mut rng = rand::rng();
	let max = u16::MAX - 25;
	let mut attempts = 100;

	loop {
		attempts -= 1;
		let port: u16 = rng.random_range(1000..max);
		let range_start = port + 3;
		let range_end = range_start + 100;
		let ports = (port, port + 1, port + 2, (range_start, range_end));

		if is_port_available(ports.0)
			&& is_port_available(ports.1)
			&& is_port_available(ports.2)
			&& (range_start..=range_end).all(is_port_available)
		{
			return Some(ports);
		}

		if attempts <= 0 {
			return None;
		}
	}
}
