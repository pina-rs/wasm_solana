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
//! 2. **Fork registration vs replay** — the shared buffer is read through
//!    forks. A fork registered *before* a frame arrives receives it directly;
//!    the pre-fix shape (cloning the never-read root fork) instead replayed the
//!    socket's entire history to reach live traffic, making subscription setup
//!    O(history). Both variants below wait for the same ack frame pushed into a
//!    queue-backed stream, so the comparison measures the same acknowledgement
//!    wait.

use std::collections::VecDeque;

use base64::Engine;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use fork_stream::StreamExt as ForkStreamExt;
use futures::StreamExt;
use serde_json::json;
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

/// A stream fed from a shared queue, so the benchmark controls exactly when
/// each frame "arrives" — modeling a websocket whose history is already
/// buffered and whose acknowledgement is pushed on demand.
struct QueuedStream {
	queue: std::sync::Arc<std::sync::Mutex<VecDeque<serde_json::Value>>>,
}

impl futures::Stream for QueuedStream {
	type Item = Result<serde_json::Value, ()>;

	fn poll_next(
		self: std::pin::Pin<&mut Self>,
		_cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Self::Item>> {
		let item = self.queue.lock().unwrap().pop_front();
		match item {
			Some(value) => std::task::Poll::Ready(Some(Ok(value))),
			None => std::task::Poll::Pending,
		}
	}
}

fn bench_fork_replay(c: &mut Criterion) {
	for history in [100_usize, 1_000, 10_000] {
		let frames = notification_frames(history);
		let ack = notification_frames(1).pop().unwrap();

		c.bench_function(
			&format!("subscription/ack_wait/replaying_root/history_{history}"),
			|b| {
				b.iter(|| {
					// The pre-fix shape: the root fork is never advanced, so
					// a fork cloned from it starts at the buffer's oldest
					// entry and must replay (and clone) every buffered frame
					// before the ack — which sits behind the whole history —
					// is visible.
					let queue = std::sync::Arc::new(std::sync::Mutex::new(
						frames
							.iter()
							.cloned()
							.chain(std::iter::once(ack.clone()))
							.collect::<VecDeque<_>>(),
					));
					let root = QueuedStream {
						queue: std::sync::Arc::clone(&queue),
					}
					.fork();
					let mut fork = root.clone();
					let found = futures::executor::block_on(fork.next());
					assert!(found.is_some());
				})
			},
		);

		// The fixed shape: the history was consumed long before this
		// subscription exists (frames are read as they arrive, so the drain
		// is amortized to zero at subscribe time — it happens in the setup
		// phase, not the measurement). The fork is then registered at the
		// live edge *before* the ack is pushed, exactly as
		// `create_subscription` now registers its ack and subscription forks
		// before sending the request, and the same ack wait reads a single
		// frame.
		c.bench_function(
			&format!("subscription/ack_wait/pre_registered_fork/history_{history}"),
			|b| {
				b.iter_batched(
					|| {
						let queue = std::sync::Arc::new(std::sync::Mutex::new(
							frames.iter().cloned().collect::<VecDeque<_>>(),
						));
						let mut root = QueuedStream {
							queue: std::sync::Arc::clone(&queue),
						}
						.fork();
						for _ in 0..history {
							futures::executor::block_on(root.next());
						}
						(root, queue)
					},
					|(mut root, queue)| {
						let mut fork = root.clone();
						queue.lock().unwrap().push_back(ack.clone());

						let found = futures::executor::block_on(fork.next());
						assert!(found.is_some());
						let _ = &mut root;
					},
					criterion::BatchSize::LargeInput,
				)
			},
		);
	}
}

criterion_group!(benches, bench_frame_parse, bench_fork_replay);
criterion_main!(benches);
