//! Benchmarks for the client's hot paths, run with the `ssr` provider so no
//! browser is required:
//!
//! ```sh
//! cargo bench -p wasm_client_solana -F ssr --bench client
//! ```
//!
//! The dispatch group measures the full request round trip through a mock
//! provider: request serialization, envelope construction, response parsing
//! into typed structs, and error probing. The wire group measures the
//! primitives underneath (wincode + base64 transaction encoding, base64
//! account decoding, base58 pubkeys).
//!
//! The mock provider's `clone` of the canned envelope is included in every
//! measurement. It is identical across variants, so deltas between variants
//! still isolate the client-side work.

use std::sync::Arc;

use async_trait::async_trait;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::Throughput;
use serde::Serialize;
use serde_json::Value;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use solana_transaction::versioned::VersionedTransaction;
use wasm_client_solana::ClientResponse;
use wasm_client_solana::ClientResult;
use wasm_client_solana::GetAccountInfoResponse;
use wasm_client_solana::GetBalanceResponse;
use wasm_client_solana::GetMultipleAccountsResponse;
use wasm_client_solana::RpcKeyedAccount;
use wasm_client_solana::RpcProvider;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::Context;
use wasm_client_solana::rpc_config::serialize_and_encode;
use wasm_client_solana::solana_account_decoder::UiAccount;
use wasm_client_solana::solana_account_decoder::UiAccountData;
use wasm_client_solana::solana_account_decoder::UiAccountEncoding;
use wasm_client_solana::solana_transaction_status::UiTransactionEncoding;

/// Returns the same envelope for every call; the client cannot tell it apart
/// from a real HTTP response.
struct MockProvider(Value);

#[async_trait]
impl RpcProvider for MockProvider {
	fn url(&self) -> String {
		"mock://benchmark".to_string()
	}

	async fn send(&self, _method: &'static str, _request: Value) -> ClientResult<Value> {
		Ok(self.0.clone())
	}
}

fn mock_client(envelope: Value) -> SolanaRpcClient {
	SolanaRpcClient::new_with_provider(
		Arc::new(MockProvider(envelope)),
		CommitmentConfig::confirmed(),
	)
}

fn envelope<T: Serialize>(result: T) -> Value {
	serde_json::to_value(ClientResponse {
		jsonrpc: "2.0".to_string(),
		result,
		id: 0,
	})
	.expect("fixture serializes")
}

fn ui_account(data_bytes: usize, seed: u8) -> UiAccount {
	let data = BASE64_STANDARD.encode(vec![seed; data_bytes]);
	UiAccount {
		lamports: 1_000_000_000,
		data: UiAccountData::Binary(data, UiAccountEncoding::Base64),
		owner: Pubkey::default(),
		executable: false,
		rent_epoch: 0,
		space: Some(data_bytes as u64),
	}
}

fn bench_dispatch(c: &mut Criterion) {
	let pubkey = Pubkey::new_unique();

	let mut group = c.benchmark_group("dispatch");
	group.sample_size(50);

	{
		let client = mock_client(envelope(GetBalanceResponse {
			context: Context { slot: 1 },
			value: 1_234_567_890,
		}));
		group.bench_function("get_balance", |b| {
			b.iter(|| {
				let pubkey = std::hint::black_box(pubkey);
				futures::executor::block_on(client.get_balance(&pubkey)).expect("balance")
			})
		});
	}

	{
		let client = mock_client(envelope(GetAccountInfoResponse {
			context: Context { slot: 1 },
			value: Some(ui_account(256, 7)),
		}));
		group.bench_function("get_account/256b", |b| {
			b.iter(|| {
				let pubkey = std::hint::black_box(pubkey);
				futures::executor::block_on(client.get_account(&pubkey)).expect("account")
			})
		});
	}

	for (count, size) in [(10, 1024), (100, 1024)] {
		let accounts: Vec<Option<UiAccount>> =
			(0..count).map(|i| Some(ui_account(size, i as u8))).collect();
		let client = mock_client(envelope(GetMultipleAccountsResponse {
			context: Context { slot: 1 },
			value: accounts,
		}));
		group.throughput(Throughput::Bytes((count * size) as u64));
		group.bench_with_input(
			BenchmarkId::new("get_multiple_accounts", format!("{count}x{size}b")),
			&(),
			|b, _| {
				b.iter(|| {
					let pubkeys = std::hint::black_box(vec![pubkey; count]);
					futures::executor::block_on(client.get_multiple_accounts(&pubkeys))
						.expect("accounts")
				})
			},
		);
	}

	{
		let count = 1000;
		let accounts: Vec<RpcKeyedAccount> = (0..count)
			.map(|i| RpcKeyedAccount {
				pubkey: Pubkey::new_unique(),
				account: ui_account(256, i as u8),
			})
			.collect();
		// `GetProgramAccountsResponse` is deserialize-only, so the envelope is
		// assembled from the raw wire shape.
		let result: Vec<serde_json::Value> = accounts
			.iter()
			.map(|keyed| {
				serde_json::json!({
					"pubkey": keyed.pubkey.to_string(),
					"account": serde_json::to_value(&keyed.account).expect("account serializes"),
				})
			})
			.collect();
		let client = mock_client(envelope(result));
		group.throughput(Throughput::Bytes((count * 256) as u64));
		group.bench_function("get_program_accounts/1000x256b", |b| {
			b.iter(|| {
				let program = std::hint::black_box(pubkey);
				futures::executor::block_on(client.get_program_accounts(&program)).expect("accounts")
			})
		});
	}

	group.finish();
}

fn bench_wire(c: &mut Criterion) {
	let mut group = c.benchmark_group("wire");

	let payer = Keypair::new();
	let recipient = Pubkey::new_unique();
	let blockhash = Hash::new_from_array([0u8; 32]);
	let transaction = Transaction::new_signed_with_payer(
		&[transfer(&payer.pubkey(), &recipient, 1_000_000)],
		Some(&payer.pubkey()),
		&[&payer],
		blockhash,
	);
	let versioned = VersionedTransaction::from(transaction);

	group.bench_function("serialize_and_encode/base64/v0", |b| {
		b.iter(|| {
			serialize_and_encode(
				std::hint::black_box(&versioned),
				UiTransactionEncoding::Base64,
			)
			.expect("encodes")
		})
	});

	let data = vec![0xAB_u8; 1024];
	let encoded = BASE64_STANDARD.encode(&data);
	let ui_data = UiAccountData::Binary(encoded, UiAccountEncoding::Base64);
	group.throughput(Throughput::Bytes(1024));
	group.bench_function("ui_account_data_decode/base64/1kb", |b| {
		b.iter(|| std::hint::black_box(&ui_data).decode().expect("decodes"))
	});

	let pubkey = Pubkey::new_unique();
	group.bench_function("pubkey_to_string/base58", |b| {
		b.iter(|| std::hint::black_box(pubkey).to_string())
	});

	group.finish();
}

criterion_group!(benches, bench_dispatch, bench_wire);
criterion_main!(benches);
