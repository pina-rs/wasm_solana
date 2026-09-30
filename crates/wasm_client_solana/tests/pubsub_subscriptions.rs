//! Native pubsub integration tests against an in-process validator.
//!
//! The browser suite covers the subscriptions a dApp actually uses; these
//! tests pin the two feeds only a validator can produce — the slot lifecycle
//! stream and raw votes — including the `--rpc-pubsub-enable-vote-subscription`
//! gate that production validators keep disabled.
#![cfg(all(feature = "ssr", not(target_arch = "wasm32")))]

use std::time::Duration;

use anyhow::Result;
use assert2::check;
use futures::StreamExt;
use test_utils_keypairs::get_wallet_keypair;
use test_utils_solana::TestValidatorRunnerProps;
use wasm_client_solana::SlotUpdate;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::prelude::*;

/// A single validator drives both subscriptions: `slotsUpdatesSubscribe`
/// reports every lifecycle stage of the slots it produces, and
/// `voteSubscribe` (opted into via the runner) reports its own votes.
#[tokio::test(flavor = "multi_thread")]
async fn slots_updates_and_vote_subscriptions() -> Result<()> {
	let runner = TestValidatorRunnerProps::builder()
		.pubkeys(vec![get_wallet_keypair().pubkey()])
		.enable_vote_subscription(true)
		.build()
		.run()
		.await;
	let rpc = runner.rpc().clone();

	// Slot updates arrive several times per slot; a handful should include
	// more than one distinct lifecycle stage.
	let subscription = rpc.slots_updates_subscribe().await?;
	let unsubscription = subscription.get_unsubscription();
	let mut stream = subscription.take(4);
	let updates = tokio::time::timeout(Duration::from_secs(30), stream.collect::<Vec<_>>())
		.await
		.expect("slot updates arrive within 30 seconds");
	unsubscription.run().await?;

	check!(updates.len() == 4);
	let results: Vec<SlotUpdate> = updates
		.into_iter()
		.map(|update| update.params.result)
		.collect();
	check!(results.iter().all(|update| {
		match update {
			SlotUpdate::FirstShredReceived { slot, .. }
			| SlotUpdate::Completed { slot, .. }
			| SlotUpdate::CreatedBank { slot, .. }
			| SlotUpdate::Frozen { slot, .. }
			| SlotUpdate::Dead { slot, .. }
			| SlotUpdate::OptimisticConfirmation { slot, .. }
			| SlotUpdate::Root { slot, .. } => *slot > 0,
		}
	}));
	check!(
		results
			.iter()
			.any(|update| matches!(update, SlotUpdate::CreatedBank { .. }))
	);

	// The validator votes on every slot it completes; one notification is
	// enough to pin the wire shape.
	let subscription = rpc.vote_subscribe().await?;
	let unsubscription = subscription.get_unsubscription();
	let mut stream = subscription.take(1);
	let votes = tokio::time::timeout(Duration::from_secs(30), stream.collect::<Vec<_>>())
		.await
		.expect("a vote arrives within 30 seconds");
	unsubscription.run().await?;

	check!(votes.len() == 1);
	let vote = votes[0].params.result.clone();
	check!(!vote.slots.is_empty());
	check!(vote.vote_pubkey != solana_pubkey::Pubkey::default());
	check!(vote.hash != solana_hash::Hash::default());
	check!(vote.signature != solana_signature::Signature::default());

	Ok(())
}

/// A dead pubsub endpoint must fail the zero-parameter subscriptions as an
/// error rather than hanging: the `?` on the connection is the only fallible
/// step in both wrappers, and it needs exercising too.
#[tokio::test(flavor = "multi_thread")]
async fn subscriptions_fail_fast_on_dead_pubsub() {
	let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
	let port = socket.local_addr().unwrap().port();
	drop(socket);
	let pubsub_url = format!("ws://127.0.0.1:{port}");
	let rpc = SolanaRpcClient::new_with_ws_and_commitment(
		"http://127.0.0.1:1",
		&pubsub_url,
		solana_commitment_config::CommitmentConfig::processed(),
	);

	check!(rpc.slots_updates_subscribe().await.is_err());
	check!(rpc.vote_subscribe().await.is_err());
}
