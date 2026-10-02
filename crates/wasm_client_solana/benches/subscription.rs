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
//! 2. **Routing** — the socket reader inspects every incoming frame once and
//!    hands it to the subscription's fan-out channel, so one slow consumer
//!    cannot tax another. The benchmark below measures that routing cost
//!    against a fully-registered subscription map.

use std::collections::HashMap;

use base64::Engine;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use futures::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;
use wasm_client_solana::GetAccountInfoResponse;
use wasm_client_solana::SUBSCRIPTION_CHANNEL_CAPACITY;
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
		owner: solana_pubkey::Pubkey::default().to_string(),
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

/// The routing step the socket reader performs for every notification: look
/// the subscription up by id and push the frame into its channel. The
/// channel's capacity exceeds the frame count, so the sends measure pure
/// routing (lookup + enqueue) with no backpressure; the drain afterwards
/// verifies nothing was dropped.
fn bench_routing(c: &mut Criterion) {
	for subscriptions in [1_usize, 8, 64] {
		let frames = notification_frames(1_000);
		let mut map: HashMap<u64, broadcast::Sender<serde_json::Value>> = HashMap::new();
		for id in 0..subscriptions as u64 {
			let (sender, _) = broadcast::channel(SUBSCRIPTION_CHANNEL_CAPACITY);
			map.insert(id, sender);
		}

		c.bench_function(
			&format!("subscription/route_1000/subs_{subscriptions}"),
			|b| {
				b.iter(|| {
					// One live receiver keeps the subscription "subscribed";
					// the frames stay buffered because 1000
					// < SUBSCRIPTION_CHANNEL_CAPACITY.
					let receivers: Vec<broadcast::Receiver<serde_json::Value>> =
						map.values().map(|sender| sender.subscribe()).collect();

					for frame in std::hint::black_box(&frames) {
						if let Some(sender) = map.get(
							&frame
								.pointer("/params/subscription")
								.and_then(serde_json::Value::as_u64)
								.unwrap_or_default(),
						) {
							let _ = sender.send(frame.clone());
						}
					}

					let buffered: usize = receivers.iter().map(|receiver| receiver.len()).sum();
					assert_eq!(buffered, 1_000);
				})
			},
		);
	}
}

criterion_group!(benches, bench_frame_parse, bench_routing);
criterion_main!(benches);
