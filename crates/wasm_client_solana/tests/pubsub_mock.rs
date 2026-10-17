//! Pubsub client behavior against a scripted mock node: every `on_*`
//! callback wrapper, the handle's lifecycle, and the fail-fast rejection
//! paths — all without booting a validator.
#![cfg(all(feature = "ssr", not(target_arch = "wasm32")))]

use std::sync::Arc;
use std::time::Duration;

use assert2::check;
use async_trait::async_trait;
use futures::SinkExt as _;
use futures::StreamExt;
use futures::channel::mpsc;
use futures_timeout::TimeoutFutureExt as _;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use solana_commitment_config::CommitmentConfig;
use solana_hash::Hash;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::versioned::VersionedTransaction;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use wasm_client_solana::BlockSubscribeRequest;
use wasm_client_solana::ClientError;
use wasm_client_solana::ClientResponse;
use wasm_client_solana::ClientResult;
use wasm_client_solana::ClientWebSocketError;
use wasm_client_solana::Context;
use wasm_client_solana::GetSignatureStatusesResponse;
use wasm_client_solana::ProgramSubscribeRequest;
use wasm_client_solana::ReceivedNotificationResponse;
use wasm_client_solana::RootSubscribeRequest;
use wasm_client_solana::RpcBlockSubscribeFilter;
use wasm_client_solana::RpcKeyedAccount;
use wasm_client_solana::RpcProvider;
use wasm_client_solana::RpcSignatureResult;
use wasm_client_solana::SignatureNotificationResponse;
use wasm_client_solana::SignatureSubscribeRequest;
use wasm_client_solana::SlotInfo;
use wasm_client_solana::SlotSubscribeRequest;
use wasm_client_solana::SlotUpdate;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::VoteNotification;
use wasm_client_solana::is_notification_for;
use wasm_client_solana::rpc_config::RpcBlockSubscribeConfig;
use wasm_client_solana::rpc_config::RpcSignatureSubscribeConfig;
use wasm_client_solana::rpc_response::BlockNotificationResponse;
use wasm_client_solana::rpc_response::RpcBlockUpdate;
use wasm_client_solana::solana_account_decoder::UiAccount;
use wasm_client_solana::solana_account_decoder::UiAccountData;
use wasm_client_solana::solana_account_decoder::UiAccountEncoding;
use wasm_client_solana::solana_transaction_status::TransactionConfirmationStatus;
use wasm_client_solana::solana_transaction_status::TransactionStatus;

/// What the mock node should do with each request it receives.
#[derive(Clone, Copy, Default)]
struct NodePolicy {
	/// Answer `*Subscribe` requests with a JSON-RPC error envelope.
	reject_subscribes: bool,
	/// Answer `*Unsubscribe` requests with a JSON-RPC error envelope.
	reject_unsubscribes: bool,
}

/// A command the test sends to the mock node's socket.
enum Command {
	/// Push a notification frame to the client.
	Push(Value),
	/// Push a binary frame that is not valid JSON, as a misbehaving node
	/// would.
	PushInvalid,
	/// Close the websocket, as a dead node would.
	Close,
}

/// A mock pubsub node: accepts the client's websocket, acknowledges (or
/// rejects) subscribe and unsubscribe requests, and forwards notification
/// frames the test scripts.
struct MockNode {
	port: u16,
	commands: mpsc::UnboundedSender<Command>,
}

impl MockNode {
	/// Push a notification frame for `subscription` carrying `result`.
	fn push<T: Serialize>(&self, method: &str, subscription: u64, result: &T) {
		let _ =
			self.commands
				.unbounded_send(Command::Push(notification(method, subscription, result)));
	}

	/// Push a binary frame carrying bytes that are not JSON.
	fn push_invalid(&self) {
		let _ = self.commands.unbounded_send(Command::PushInvalid);
	}

	/// End the connection.
	fn close(&self) {
		let _ = self.commands.unbounded_send(Command::Close);
	}
}

/// Start the mock node and a client whose pubsub transport points at it.
async fn mock_node(policy: NodePolicy) -> (MockNode, SolanaRpcClient) {
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let port = listener.local_addr().unwrap().port();
	let (sender, mut commands) = mpsc::unbounded::<Command>();

	tokio::spawn(async move {
		let (stream, _) = listener.accept().await.unwrap();
		let websocket = tokio_tungstenite::accept_async(stream).await.unwrap();
		let (mut sink, mut read) = websocket.split();
		let mut subscription_id = 0_u64;

		loop {
			tokio::select! {
				command = commands.next() => match command {
					Some(Command::Push(frame)) => {
						let _ = sink.send(Message::text(frame.to_string())).await;
					}
					Some(Command::PushInvalid) => {
						let _ = sink.send(Message::binary(Vec::from([0xff_u8, 0xfe]))).await;
					}
					Some(Command::Close) | None => {
						let _ = sink.send(Message::Close(None)).await;
						break;
					}
				},
				frame = read.next() => match frame {
					Some(Ok(Message::Text(text))) => {
						let reply = match serde_json::from_str::<Value>(text.as_str()) {
							Ok(request) => {
								let id = request["id"].clone();
								let method = request["method"].as_str().unwrap_or_default();

								let rejected = if method.ends_with("Subscribe") {
									policy.reject_subscribes
								} else {
									policy.reject_unsubscribes
								};

								if rejected {
									json!({
										"jsonrpc": "2.0",
										"error": { "code": -32602, "message": "mock rejection" },
										"id": id,
									})
								} else if method.ends_with("Subscribe") {
									let acknowledgement = json!({
										"jsonrpc": "2.0",
										"result": subscription_id,
										"id": id,
									});

									subscription_id += 1;

									acknowledgement
								} else {
									json!({ "jsonrpc": "2.0", "result": true, "id": id })
								}
							}
							Err(_) => continue,
						};

						let _ = sink.send(Message::text(reply.to_string())).await;
					}
					Some(Ok(_)) => continue,
					_ => break,
				},
			}
		}
	});

	let client = SolanaRpcClient::new_with_ws_and_commitment(
		"http://127.0.0.1:1",
		&format!("ws://127.0.0.1:{port}"),
		CommitmentConfig::confirmed(),
	);

	(
		MockNode {
			port,
			commands: sender,
		},
		client,
	)
}

/// Wrap `future` so a stuck handshake fails the test instead of hanging it.
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn notification<T: Serialize>(method: &str, subscription: u64, result: &T) -> Value {
	json!({
		"jsonrpc": "2.0",
		"method": method,
		"params": { "result": serde_json::to_value(result).expect("payload serializes"), "subscription": subscription },
	})
}

/// `on_root_change` delivers root notifications and unsubscribes cleanly.
#[tokio::test]
async fn on_root_change_delivers_root_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_root_change(RootSubscribeRequest::builder().build(), move |root| {
			let _ = sender.unbounded_send(root);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(subscription.subscription_id() == 0);
	check!(subscription.id() >= 1000);
	check!(!subscription.is_stopped());

	node.push("rootNotification", 0, &json!(1234));

	let root = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(root.0 == 1234);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
	check!(node.port > 0);
}

/// `on_slots_updates` delivers the slot lifecycle notifications.
#[tokio::test]
async fn on_slots_updates_delivers_lifecycle_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_slots_updates(move |update| {
			let _ = sender.unbounded_send(update);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push(
		"slotsUpdatesNotification",
		0,
		&SlotUpdate::FirstShredReceived {
			slot: 5,
			timestamp: 1,
		},
	);

	let update = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(
		update
			== SlotUpdate::FirstShredReceived {
				slot: 5,
				timestamp: 1,
			}
	);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// `on_vote` delivers gossiped votes.
#[tokio::test]
async fn on_vote_delivers_vote_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_vote(move |vote| {
			let _ = sender.unbounded_send(vote);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let vote = VoteNotification {
		vote_pubkey: Pubkey::default(),
		slots: vec![7, 8],
		hash: Hash::new_from_array([1; 32]),
		timestamp: Some(4),
		signature: Signature::default(),
	};

	node.push("voteNotification", 0, &vote);

	let received = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(received == vote);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// `on_signature` delivers the watched signature's final status.
#[tokio::test]
async fn on_signature_delivers_signature_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_signature(
			SignatureSubscribeRequest::builder()
				.signature(Signature::default())
				.config(RpcSignatureSubscribeConfig::default())
				.build(),
			move |status| {
				let _ = sender.unbounded_send(status);
			},
		)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push(
		"signatureNotification",
		0,
		&SignatureNotificationResponse {
			context: Context { slot: 3 },
			value: RpcSignatureResult { err: None, slot: 2 },
		},
	);

	let status = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(status.value.err.is_none());

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// `on_program_change` delivers keyed-account notifications.
#[tokio::test]
async fn on_program_change_delivers_program_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_program_change(
			ProgramSubscribeRequest::builder()
				.program_id(Pubkey::default())
				.build(),
			move |accounts| {
				let _ = sender.unbounded_send(accounts);
			},
		)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let account = UiAccount {
		lamports: 42,
		data: UiAccountData::Binary(String::new(), UiAccountEncoding::Base64),
		owner: Pubkey::default().to_string(),
		executable: false,
		rent_epoch: 0,
		space: Some(0),
	};

	node.push(
		"programNotification",
		0,
		&json!([RpcKeyedAccount {
			pubkey: Pubkey::default(),
			account,
		}]),
	);

	let accounts = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let keyed = accounts.keyed_accounts().expect("one keyed account");

	check!(keyed.len() == 1);
	check!(keyed[0].account.lamports == 42);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// `on_block` delivers block notifications for the block feed.
#[tokio::test]
async fn on_block_delivers_block_notifications() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_block(
			BlockSubscribeRequest::builder()
				.filter(RpcBlockSubscribeFilter::All)
				.config(RpcBlockSubscribeConfig::default())
				.build(),
			move |block| {
				let _ = sender.unbounded_send(block);
			},
		)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push(
		"blockNotification",
		0,
		&BlockNotificationResponse {
			context: Context { slot: 9 },
			value: RpcBlockUpdate {
				slot: 9,
				block: None,
				err: None,
			},
		},
	);

	let block = receiver
		.next()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(block.value.slot == 9);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// A detached handle leaves the pump running: notifications keep arriving
/// after the handle is gone, and no unsubscription is ever sent.
#[tokio::test]
async fn detached_handle_keeps_delivering() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, mut receiver) = mpsc::unbounded();

	let subscription = client
		.on_root_change(RootSubscribeRequest::builder().build(), move |root| {
			let _ = sender.unbounded_send(root);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	subscription.detach();

	node.push("rootNotification", 0, &json!(11));
	node.push("rootNotification", 0, &json!(12));

	let mut roots = Vec::new();
	for _ in 0..2 {
		roots.push(
			receiver
				.next()
				.timeout(TEST_TIMEOUT)
				.await
				.unwrap()
				.unwrap()
				.0,
		);
	}

	check!(roots == vec![11, 12]);
}

/// When the socket ends, the pump reports stopped through `is_stopped`, and
/// a later `unsubscribe` resolves `Ok` — there is nothing to unsubscribe
/// from anymore.
#[tokio::test]
async fn stopped_pump_reports_and_unsubscribes_cleanly() {
	let (node, client) = mock_node(NodePolicy::default()).await;
	let (sender, _receiver) = mpsc::unbounded::<u64>();

	let subscription = client
		.on_root_change(RootSubscribeRequest::builder().build(), move |root| {
			let _ = sender.unbounded_send(root.0);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.close();

	let mut stopped = false;
	for _ in 0..100 {
		if subscription.is_stopped() {
			stopped = true;
			break;
		}

		tokio::time::sleep(Duration::from_millis(20)).await;
	}

	check!(stopped);

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// A node that rejects the subscription fails the subscribe immediately —
/// no waiting for an acknowledgement that will never come.
#[tokio::test]
async fn rejected_subscription_fails_fast() {
	let (_node, client) = mock_node(NodePolicy {
		reject_subscribes: true,
		reject_unsubscribes: false,
	})
	.await;

	let error = client
		.on_root_change(RootSubscribeRequest::builder().build(), |root| {
			let _ = std::hint::black_box(root);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap_err();

	match error {
		ClientError::WebSocket(ClientWebSocketError::Subscription) => {}
		error => panic!("expected a subscription rejection, got {error:?}"),
	}
}

/// A node that rejects the unsubscription surfaces the rejection through
/// `unsubscribe`.
#[tokio::test]
async fn rejected_unsubscription_fails_fast() {
	let (node, client) = mock_node(NodePolicy {
		reject_subscribes: false,
		reject_unsubscribes: true,
	})
	.await;
	let (sender, _receiver) = mpsc::unbounded::<u64>();

	let subscription = client
		.on_root_change(RootSubscribeRequest::builder().build(), move |root| {
			let _ = sender.unbounded_send(root.0);
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push("rootNotification", 0, &json!(1));

	let error = subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap_err();

	match error {
		ClientWebSocketError::Unsubscription => {}
		error => panic!("expected an unsubscription rejection, got {error:?}"),
	}
}

/// The frame-matching probe behind subscription dispatch: a frame belongs to
/// a subscription only when both the method and the subscription id match.
#[test]
fn frame_matching_probe() {
	let frame = notification("rootNotification", 3, &7_u64);

	check!(is_notification_for(&frame, "rootNotification", 3));
	check!(!is_notification_for(&frame, "rootNotification", 4));
	check!(!is_notification_for(&frame, "slotNotification", 3));
	check!(!is_notification_for(
		&json!({"jsonrpc": "2.0", "result": 0, "id": 1}),
		"rootNotification",
		3,
	));
	check!(!is_notification_for(
		&notification(
			"logsNotification",
			9,
			&wasm_client_solana::rpc_response::LogsNotificationResponse {
				context: Context { slot: 1 },
				value: wasm_client_solana::rpc_response::RpcLogsResponse {
					signature: Signature::default(),
					err: None,
					logs: vec!["program log: hi".to_string()],
				},
			},
		),
		"rootNotification",
		3,
	));
}

/// A received-notification frame does not match a signature subscription's
/// confirmation feed — the probe skips it by method.
#[test]
fn received_notification_frame_is_skipped_by_method() {
	let frame = notification(
		"receivedNotification",
		5,
		&ReceivedNotificationResponse {
			context: Context { slot: 1 },
			value: Signature::default(),
		},
	);

	check!(!is_notification_for(&frame, "signatureNotification", 5));
	check!(!is_notification_for(
		&json!({"jsonrpc": "2.0", "method": "slotNotification"}),
		"slotNotification",
		5,
	));
}

/// The stream flavor of every feed the callback sweep does not already
/// cover, and a stray frame racing the handshake: the acknowledgement wait
/// must skip frames that answer no request of ours.
#[tokio::test]
async fn stream_subscriptions_cover_the_remaining_feeds() {
	let (node, client) = mock_node(NodePolicy::default()).await;

	// The frames are queued before any subscription exists, so they land on
	// the socket while the first handshake's acknowledgement is still in
	// flight: a notification answering no request, and a binary frame that
	// is not valid JSON at all.
	node.push("rootNotification", 99, &json!(1));
	node.push_invalid();

	let mut slot = client
		.slot_subscribe(SlotSubscribeRequest::builder().build())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push(
		"slotNotification",
		0,
		&SlotInfo {
			parent: 0,
			slot: 1,
			timestamp: None,
		},
	);

	let slot_info = slot.next().timeout(TEST_TIMEOUT).await.unwrap().unwrap();

	check!(slot_info.params.result.slot == 1);

	// The invalid frame also races the unsubscription's acknowledgement wait.
	node.push_invalid();

	slot.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let mut root = client
		.root_subscribe(RootSubscribeRequest::builder().build())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push("rootNotification", 1, &json!(2));

	let root_slot = root.next().timeout(TEST_TIMEOUT).await.unwrap().unwrap();

	check!(root_slot.params.result.0 == 2);

	root.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let mut block = client
		.block_subscribe(
			BlockSubscribeRequest::builder()
				.filter(RpcBlockSubscribeFilter::All)
				.build(),
		)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push(
		"blockNotification",
		2,
		&BlockNotificationResponse {
			context: Context { slot: 3 },
			value: RpcBlockUpdate {
				slot: 3,
				block: None,
				err: None,
			},
		},
	);

	let block_update = block.next().timeout(TEST_TIMEOUT).await.unwrap().unwrap();

	check!(block_update.params.result.value.slot == 3);

	block
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let mut program = client
		.program_subscribe(
			ProgramSubscribeRequest::builder()
				.program_id(Pubkey::default())
				.build(),
		)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	let account = UiAccount {
		lamports: 42,
		data: UiAccountData::Binary(String::new(), UiAccountEncoding::Base64),
		owner: Pubkey::default().to_string(),
		executable: false,
		rent_epoch: 0,
		space: Some(0),
	};

	node.push(
		"programNotification",
		3,
		&json!([RpcKeyedAccount {
			pubkey: Pubkey::default(),
			account,
		}]),
	);

	let accounts = program.next().timeout(TEST_TIMEOUT).await.unwrap().unwrap();

	let keyed = accounts
		.params
		.result
		.keyed_accounts()
		.expect("one keyed account");

	check!(keyed.len() == 1);
	check!(keyed[0].account.lamports == 42);

	program
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}

/// An HTTP transport answering `sendTransaction` with the transaction's own
/// signature, so the send path round-trips without a node.
struct SendProvider {
	envelope: Value,
}

#[async_trait]
impl RpcProvider for SendProvider {
	fn url(&self) -> String {
		"http://127.0.0.1:1".to_string()
	}

	async fn send(&self, _method: &'static str, _request: Value) -> ClientResult<Value> {
		Ok(self.envelope.clone())
	}
}

/// `send_transaction` returns the signature the node echoed back, which for
/// a healthy node is the transaction's own first signature.
#[tokio::test]
async fn send_transaction_returns_the_node_signature() {
	let transaction = VersionedTransaction {
		signatures: vec![Signature::default()],
		message: solana_message::VersionedMessage::V0(solana_message::v0::Message::default()),
	};

	let client = SolanaRpcClient::new_with_provider(
		Arc::new(SendProvider {
			envelope: json!({
				"jsonrpc": "2.0",
				"id": 0,
				"result": Signature::default().to_string(),
			}),
		}),
		CommitmentConfig::confirmed(),
	);

	let signature = client
		.send_transaction(&transaction)
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(signature == Signature::default());
}

/// An HTTP transport answering every request with one canned
/// `getSignatureStatuses` envelope, for driving the confirmation paths
/// deterministically.
struct StatusProvider {
	url: String,
	envelope: Value,
}

#[async_trait]
impl RpcProvider for StatusProvider {
	fn url(&self) -> String {
		self.url.clone()
	}

	async fn send(&self, _method: &'static str, _request: Value) -> ClientResult<Value> {
		Ok(self.envelope.clone())
	}
}

/// A client whose pubsub transport is the mock node and whose HTTP transport
/// always reports `status`. The provider URL's port sits one below the node's
/// websocket port so the derived pubsub URL (port + 1) points at the node.
async fn client_with_status(status: TransactionStatus) -> (MockNode, SolanaRpcClient) {
	let (node, _client) = mock_node(NodePolicy::default()).await;
	let client = SolanaRpcClient::new_with_provider(
		Arc::new(StatusProvider {
			url: format!("http://127.0.0.1:{}", node.port - 1),
			envelope: serde_json::to_value(&ClientResponse {
				jsonrpc: "2.0".to_string(),
				result: GetSignatureStatusesResponse {
					context: Context { slot: 1 },
					value: vec![Some(status)],
				},
				id: 0,
			})
			.expect("envelope serializes"),
		}),
		CommitmentConfig::confirmed(),
	);

	(node, client)
}

fn status(confirmation: Option<TransactionConfirmationStatus>) -> TransactionStatus {
	TransactionStatus {
		slot: 5,
		confirmations: None,
		status: Ok(()),
		err: None,
		confirmation_status: confirmation,
	}
}

/// A status poll that already reached the target commitment confirms through
/// the covering poll, without waiting for a notification.
#[tokio::test]
async fn confirmation_lands_through_the_covering_status_poll() {
	let (_node, client) =
		client_with_status(status(Some(TransactionConfirmationStatus::Confirmed))).await;

	let confirmed = client
		.confirm_transaction_with_commitment(&Signature::default(), CommitmentConfig::confirmed())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(confirmed);
}

/// A status without a confirmation confirmation status — some nodes omit it
/// for very old transactions — keeps waiting, and the signature notification
/// confirms instead.
#[tokio::test]
async fn confirmation_lands_through_the_notification_when_the_poll_lags() {
	let (node, client) = client_with_status(status(None)).await;

	// The notification may be queued before the internal subscription
	// exists: the pre-registered fork still captures it.
	node.push(
		"signatureNotification",
		0,
		&SignatureNotificationResponse {
			context: Context { slot: 3 },
			value: RpcSignatureResult { err: None, slot: 2 },
		},
	);

	let confirmed = client
		.confirm_transaction_with_commitment(&Signature::default(), CommitmentConfig::confirmed())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(confirmed);
}

/// A finalized target with a merely confirmed status does not count as
/// reached; the notification carries the confirmation.
#[tokio::test]
async fn finalized_target_waits_for_the_notification() {
	let (node, client) =
		client_with_status(status(Some(TransactionConfirmationStatus::Confirmed))).await;

	node.push(
		"signatureNotification",
		0,
		&SignatureNotificationResponse {
			context: Context { slot: 3 },
			value: RpcSignatureResult { err: None, slot: 2 },
		},
	);

	let confirmed = client
		.confirm_transaction_with_commitment(&Signature::default(), CommitmentConfig::finalized())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(confirmed);
}

/// `Processed` is the lowest commitment level, so any reported status
/// satisfies it through the covering poll.
#[tokio::test]
async fn processed_target_confirms_through_the_poll() {
	let (_node, client) =
		client_with_status(status(Some(TransactionConfirmationStatus::Processed))).await;

	let confirmed = client
		.confirm_transaction_with_commitment(&Signature::default(), CommitmentConfig::processed())
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	check!(confirmed);
}

/// A panicking callback kills its pump without a parting report; the handle
/// still unsubscribes to a clean `Ok` because there is nothing left to lose.
#[tokio::test]
async fn panicking_callback_still_unsubscribes_cleanly() {
	let (node, client) = mock_node(NodePolicy::default()).await;

	let subscription = client
		.on_root_change(RootSubscribeRequest::builder().build(), |root| {
			panic!("callback explodes on {root:?}");
		})
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();

	node.push("rootNotification", 0, &json!(1));

	// Let the panic land before unsubscribing.
	tokio::time::sleep(Duration::from_millis(200)).await;

	subscription
		.unsubscribe()
		.timeout(TEST_TIMEOUT)
		.await
		.unwrap()
		.unwrap();
}
