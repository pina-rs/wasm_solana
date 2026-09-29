//! Benchmarks for the pubsub path's CPU costs, run with:
//!
//! ```sh
//! cargo bench -p wasm_client_solana -F ssr --bench subscription
//! ```
//!
//! Two costs dominate real sockets:
//!
//! 1. **Frame parsing** — every notification is parsed from a JSON value into
//!    `SubscriptionResponse<T>`; this is the per-notification tax a busy
//!    `programSubscribe` pays.
//! 2. **Fork replay** — `WebSocketProvider` shares one forked stream. A fork
//!    cloned from an idle root starts at the root's offset (offset 0 without a
//!    drain), so a new subscription re-reads and re-parses every buffered frame
//!    before reaching live traffic: setup cost is O(history). The "root
//!    advanced" variant models the fix (a task that keeps the root fork at the
//!    live edge), where a new fork starts at O(1).

use base64::Engine;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use fork_stream::StreamExt as ForkStreamExt;
use futures::StreamExt;
use serde_json::json;
use wasm_client_solana::Context;
use wasm_client_solana::GetAccountInfoResponse;
use wasm_client_solana::SubscriptionResponse;
use wasm_client_solana::solana_account_decoder::UiAccount;
use wasm_client_solana::solana_account_decoder::UiAccountData;
use wasm_client_solana::solana_account_decoder::UiAccountEncoding;

fn notification_frames(count: usize) -> Vec<serde_json::Value> {
	let account = UiAccount {
		lamports: 1_000_000_000,
		data: UiAccountData::Binary(
			base64::prelude::BASE64_STANDARD.encode(vec![0u8; 64]),
			UiAccountEncoding::Base64,
		),
		owner: solana_pubkey::Pubkey::default(),
		executable: false,
		rent_epoch: 0,
		space: Some(64),
	};

	(0..count)
		.map(|slot| {
			json!({
				"jsonrpc": "2.0",
				"method": "accountNotification",
				"params": {
					"result": {
						"context": { "slot": slot },
						"value": account.clone(),
					},
					"subscription": 0,
				}
			})
		})
		.collect()
}

fn bench_frame_parse(c: &mut Criterion) {
	let frames = notification_frames(1_000);
	c.bench_function("subscription/parse_1000_notifications", |b| {
		b.iter(|| {
			let frames = std::hint::black_box(&frames);
			for frame in frames {
				let parsed: SubscriptionResponse<GetAccountInfoResponse> =
					serde_json::from_value(std::hint::black_box(frame.clone()))
						.expect("notification parses");
				std::hint::black_box(&parsed);
			}
		})
	});
}

fn bench_fork_replay(c: &mut Criterion) {
	for history in [100_usize, 1_000, 10_000] {
		let frames = notification_frames(history);

		c.bench_function(
			&format!("subscription/ack_wait/replaying_root/history_{history}"),
			|b| {
				b.iter(|| {
					// The pre-fix shape: the root fork is never polled, so it
					// stays at offset 0 and every new fork replays the whole
					// buffer to find the ack at the live edge.
					let root = futures::stream::iter(std::hint::black_box(frames.clone()))
						.map(Ok::<_, ()>)
						.fork();
					let mut fork = root.clone();
					let ack = futures::executor::block_on(fork.next());
					assert!(ack.is_some());
				})
			},
		);

		// The fixed shape: a background drain task keeps the root at the
		// live edge (amortized, outside the ack wait), so a new fork starts
		// where traffic is live and the ack wait reads O(1) frames. The
		// drain runs in the setup phase, mirroring production where it is a
		// long-lived task rather than part of any single subscription.
		c.bench_function(
			&format!("subscription/ack_wait/root_at_live_edge/history_{history}"),
			|b| {
				b.iter_batched(
					|| {
						let mut root = futures::stream::iter(std::hint::black_box(frames.clone()))
							.map(Ok::<_, ()>)
							.fork();
						while futures::executor::block_on(root.next()).is_some() {}
						root
					},
					|mut root| {
						let mut fork = root.clone();
						// The source is exhausted; the fork yields nothing new
						// — its starting offset is already the live edge.
						let ack = futures::executor::block_on(fork.next());
						assert!(ack.is_none());
					},
					criterion::BatchSize::SmallInput,
				)
			},
		);
	}
}

criterion_group!(benches, bench_frame_parse, bench_fork_replay);
criterion_main!(benches);
