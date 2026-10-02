use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

#[cfg(feature = "ssr")]
use self::websocket_provider_reqwest::*;
#[cfg(not(feature = "ssr"))]
use self::websocket_provider_wasm::*;
use crate::ClientRequest;
use crate::ClientWebSocketError;
use crate::SubscriptionId;
use crate::SubscriptionResponse;
use crate::SubscriptionResult;
use crate::UnsubscriptionResult;
use crate::WebSocketMethod;
use crate::WebSocketNotification;
use crate::utils::get_ws_url;

/// Notifications buffered per subscription before the slowest consumer is
/// marked as lagging.
///
/// One stalled subscriber used to stall the single shared buffer every
/// subscription forked from; now each subscription has its own bounded
/// channel, so a consumer that cannot keep up drops frames (surfaced
/// through [`Subscription::missed_notifications`]) instead of growing
/// without bound or blocking the socket's other subscribers.
pub const SUBSCRIPTION_CHANNEL_CAPACITY: usize = 1024;

/// Parse an incoming websocket frame as JSON, so both platform backends can
/// yield [`Value`]s from their [`Stream`] halves regardless of whether the
/// node sent text or binary frames.
pub trait ToWebSocketValue {
	/// Decode this frame into a [`Value`], failing with
	/// [`ClientWebSocketError::InvalidMessage`] when it is neither text nor
	/// binary, or is not valid JSON.
	fn to_websocket_value(&self) -> Result<Value, ClientWebSocketError>;
}

impl<T, E> ToWebSocketValue for Result<T, E>
where
	T: ToWebSocketValue,
	E: Into<ClientWebSocketError>,
	for<'a> &'a E: Into<ClientWebSocketError>,
{
	fn to_websocket_value(&self) -> Result<Value, ClientWebSocketError> {
		match self {
			Ok(value) => Ok(value.to_websocket_value()?),
			Err(err) => Err(err.into()),
		}
	}
}

/// A response still waiting for its `id` to come back over the socket.
enum PendingRequest {
	/// A subscribe request. The channel becomes the subscription's fan-out
	/// as soon as the acknowledgement reveals the server-assigned
	/// subscription id; registering it while handling the ack (rather than
	/// in the caller) is what guarantees no notification racing the
	/// handshake can be dropped.
	Subscribe {
		done: oneshot::Sender<Value>,
		channel: broadcast::Sender<Value>,
	},
	/// Any other request matched by id, such as an unsubscription.
	Ack { done: oneshot::Sender<Value> },
}

/// Bookkeeping shared between the provider handle, its subscriptions, and
/// the socket reader task that feeds them.
struct SharedSocket {
	/// The socket's write half; one lock guards the single writer.
	sink: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	/// Allocated request ids, shared by every handle.
	next_id: std::sync::Mutex<u32>,
	/// Responses that have been requested but not yet answered, by id.
	pending: std::sync::Mutex<HashMap<u32, PendingRequest>>,
	/// Fan-out channel per live subscription, keyed by the server-assigned
	/// subscription id found in each notification.
	subscriptions: std::sync::Mutex<HashMap<SubscriptionId, broadcast::Sender<Value>>>,
}

impl SharedSocket {
	/// Allocate a fresh JSON-RPC request id.
	fn allocate_id(&self) -> Result<u32, ClientWebSocketError> {
		let mut id = self
			.next_id
			.lock()
			.map_err(|_| ClientWebSocketError::ConnectionError)?;
		let current = *id;
		*id += 1;

		Ok(current)
	}

	/// Write one request to the socket.
	async fn send(&self, request: Value) -> Result<(), ClientWebSocketError> {
		let mut lock = self.sink.lock().await;
		lock.send(request)
			.await
			.map_err(|_| ClientWebSocketError::MessageSendError)
	}

	/// Send a request that expects a response matched by id, and wait for
	/// that response envelope.
	async fn request(&self, method: &str, params: Value) -> Result<Value, ClientWebSocketError> {
		let id = self.allocate_id()?;
		let request = ClientRequest::builder()
			.method(method)
			.params(params)
			.id(id)
			.build()
			.try_to_value()?;
		let (done, ack) = oneshot::channel();
		self.pending
			.lock()
			.map_err(|_| ClientWebSocketError::ConnectionError)?
			.insert(id, PendingRequest::Ack { done });

		if let Err(error) = self.send(request).await {
			self.forget_pending(id);
			return Err(error);
		}

		// The reader drops the sender when the socket ends, failing this
		// wait instead of parking it forever.
		ack.await.map_err(|_| ClientWebSocketError::ConnectionError)
	}

	/// Drop a pending response registration, for request paths that fail
	/// before the answer could arrive.
	fn forget_pending(&self, id: u32) {
		if let Ok(mut pending) = self.pending.lock() {
			pending.remove(&id);
		}
	}
}

/// Deliver one decoded frame: responses go to the caller waiting on their
/// id; notifications go to the channel registered for their subscription.
///
/// Pure with respect to the maps it is handed, so the routing rules are
/// unit-testable without a socket.
fn route(
	value: Value,
	pending: &mut HashMap<u32, PendingRequest>,
	subscriptions: &mut HashMap<SubscriptionId, broadcast::Sender<Value>>,
) {
	let response_id = value.get("id").and_then(Value::as_u64);
	if let Some(response_id) = response_id
		&& let Ok(response_id) = u32::try_from(response_id)
		&& let Some(request) = pending.remove(&response_id)
	{
		match request {
			PendingRequest::Subscribe { done, channel } => {
				if let Some(subscription_id) = envelope_subscription_id(&value) {
					subscriptions.insert(subscription_id, channel);
				}
				// A rejected subscribe carries no result: the channel drops
				// here, closing the caller's receiver immediately.
				let _ = done.send(value);
			}
			PendingRequest::Ack { done } => {
				let _ = done.send(value);
			}
		}

		return;
	}

	let subscription_id = value
		.pointer("/params/subscription")
		.and_then(Value::as_u64);
	if let Some(subscription_id) = subscription_id
		&& let Some(channel) = subscriptions.get(&subscription_id)
	{
		// A send only fails when every receiver is gone, which means nobody
		// is left to read this subscription anyway.
		let _ = channel.send(value);
	}
}

/// The subscription id inside a subscribe acknowledgement, if the envelope
/// carries one.
fn envelope_subscription_id(value: &Value) -> Option<SubscriptionId> {
	if value.get("error").is_some() {
		return None;
	}

	value.get("result")?.as_u64()
}

/// Spawn a future on the runtime available to the target.
///
/// Mirrors [`crate::utils::spawn_local`] except that `ssr` uses
/// [`tokio::task::spawn`]: the reader and the fire-and-forget unsubscriptions
/// must not require a `LocalSet`.
#[cfg(feature = "js")]
fn spawn_future<F: Future<Output = ()> + 'static>(fut: F) {
	wasm_bindgen_futures::spawn_local(fut);
}

#[cfg(all(feature = "ssr", not(feature = "js")))]
fn spawn_future<F: Future<Output = ()> + Send + 'static>(fut: F) {
	tokio::task::spawn(fut);
}

#[cfg(not(any(feature = "js", feature = "ssr")))]
fn spawn_future<F: Future<Output = ()> + 'static>(fut: F) {
	futures::executor::block_on(fut);
}

/// Drive the socket's read half until it ends, routing every frame.
fn spawn_reader(stream: SplitStream<WebSocketStream>, shared: Arc<SharedSocket>) {
	spawn_future(async move {
		let mut stream = stream;
		while let Some(result) = stream.next().await {
			match result {
				Ok(value) => {
					let Ok(mut pending) = shared.pending.lock() else {
						break;
					};
					let Ok(mut subscriptions) = shared.subscriptions.lock() else {
						break;
					};
					route(value, &mut pending, &mut subscriptions);
				}
				Err(_) => break,
			}
		}

		// The socket ended: fail every outstanding wait and close every
		// subscription stream rather than parking consumers forever.
		if let Ok(mut pending) = shared.pending.lock() {
			pending.clear();
		}
		if let Ok(mut subscriptions) = shared.subscriptions.lock() {
			subscriptions.clear();
		}
	});
}

/// A connection to a Solana node's pubsub endpoint.
///
/// Cloning is cheap and every clone shares one socket: the sink is guarded by
/// a mutex and a single reader task routes every incoming frame to the
/// subscription it belongs to. That sharing is what lets many
/// [`Subscription`]s — plus their unsubscriptions — coexist on a single
/// websocket, which browsers limit the number of.
///
/// The reader retains nothing beyond each subscription's bounded channel, so
/// memory stays proportional to live subscriptions rather than to the total
/// number of frames the connection has ever received.
#[derive(Clone, derive_more::Debug)]
pub struct WebSocketProvider {
	/// The websocket url.
	url: String,
	#[debug(skip)]
	shared: Arc<SharedSocket>,
}

impl WebSocketProvider {
	/// Open a pubsub connection, deriving the websocket endpoint from an RPC
	/// URL: the scheme is rewritten from `http(s)` to `ws(s)` and any explicit
	/// port is bumped by one, matching the agave convention of serving pubsub
	/// one port above RPC.
	pub fn new(url: impl Into<String>) -> Self {
		let url = get_ws_url(url);
		let stream = WebSocketStream::new(&url);
		let (sink, stream) = stream.split();
		let shared = Arc::new(SharedSocket {
			sink: Arc::new(Mutex::new(sink)),
			// start with 1000 since the default id used for http methods is 0
			next_id: std::sync::Mutex::new(1000),
			pending: std::sync::Mutex::new(HashMap::new()),
			subscriptions: std::sync::Mutex::new(HashMap::new()),
		});

		spawn_reader(stream, Arc::clone(&shared));

		Self { url, shared }
	}

	/// The websocket endpoint URL, after the http-to-ws rewrite done at
	/// construction.
	pub fn url(&self) -> &str {
		&self.url
	}

	/// Create a subscription and return the `id` used to create the
	/// subscription, the server-assigned `subscription_id`, and a receiver
	/// already positioned to see every notification of that subscription —
	/// hand it to [`Subscription::from_parts`].
	///
	/// The fan-out channel is registered by the socket reader while it
	/// handles the acknowledgement, so a notification racing the handshake
	/// is still delivered: the reader processes it strictly after the ack
	/// that assigned the id.
	pub async fn create_subscription<T: WebSocketMethod>(
		&self,
		params: T,
	) -> Result<(u32, SubscriptionId, broadcast::Receiver<Value>), ClientWebSocketError> {
		let id = self.shared.allocate_id()?;
		let request = ClientRequest::builder()
			.method(T::SUBSCRIBE)
			.params(params)
			.id(id)
			.build()
			.try_to_value()?;

		// The receiver exists before the request is even sent, so there is
		// no window in which the reader could send into a channel with no
		// receiver and drop a frame.
		let (channel, receiver) = broadcast::channel(SUBSCRIPTION_CHANNEL_CAPACITY);
		let (done, ack) = oneshot::channel();
		{
			let mut pending = self
				.shared
				.pending
				.lock()
				.map_err(|_| ClientWebSocketError::ConnectionError)?;
			pending.insert(id, PendingRequest::Subscribe { done, channel });
		}

		if let Err(error) = self.shared.send(request).await {
			self.shared.forget_pending(id);
			return Err(error);
		}

		let envelope = ack.await.map_err(|_| ClientWebSocketError::Subscription)?;
		let response: SubscriptionResult =
			serde_json::from_value(envelope).map_err(|_| ClientWebSocketError::Subscription)?;

		Ok((id, response.result, receiver))
	}
}

/// Created from a [`Subscription`] to send a message to unsubscribe.
#[derive(Clone)]
pub struct Unsubscription {
	/// The name of the method used to unsubscribe.
	method: &'static str,
	shared: Arc<SharedSocket>,
	/// Set once an unsubscription has actually been sent, so the
	/// subscription's own `Drop` does not fire a second request.
	unsubscribed: Arc<AtomicBool>,
	/// The `subscription_id` used to unsubscribe.
	subscription_id: SubscriptionId,
}

impl PartialEq for Unsubscription {
	fn eq(&self, other: &Self) -> bool {
		self.method.eq(other.method) && self.subscription_id.eq(&other.subscription_id)
	}
}

impl Eq for Unsubscription {}

impl Hash for Unsubscription {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.method.hash(state);
		self.subscription_id.hash(state);
	}
}

impl Unsubscription {
	/// Send the unsubscribe request and wait for the node's ack.
	///
	/// Consumes `self`. Errors if the request cannot be written or the
	/// connection ends before the ack arrives.
	pub async fn run(self) -> Result<(), ClientWebSocketError> {
		let envelope = self
			.shared
			.request(self.method, serde_json::json!([self.subscription_id]))
			.await?;
		let response: UnsubscriptionResult =
			serde_json::from_value(envelope).map_err(|_| ClientWebSocketError::Unsubscription)?;

		if !response.result {
			return Err(ClientWebSocketError::Unsubscription);
		}

		if let Ok(mut subscriptions) = self.shared.subscriptions.lock() {
			subscriptions.remove(&self.subscription_id);
		}
		self.unsubscribed.store(true, Ordering::Relaxed);

		Ok(())
	}
}

/// A [`Subscription`] is used to manage a solana websocket rpc method.
///
/// Dropping the last handle unsubscribes: a fire-and-forget request is sent
/// on the shared socket so the node stops delivering frames for it. Call
/// [`Subscription::unsubscribe`](Self::unsubscribe) (or run an
/// [`Unsubscription`]) instead when the ack matters.
#[derive(derive_more::Debug)]
pub struct Subscription<T: DeserializeOwned + WebSocketNotification> {
	/// This handle's own view of the subscription's fan-out, as a
	/// [`Stream`](futures::Stream) of raw frames.
	receiver: BroadcastStream<Value>,
	#[debug(skip)]
	shared: Arc<SharedSocket>,
	/// Cleared by an explicit unsubscription so `Drop` does not send a
	/// second request.
	#[debug(skip)]
	unsubscribe_on_drop: Arc<AtomicBool>,
	/// Notifications dropped because this handle fell
	/// [`SUBSCRIPTION_CHANNEL_CAPACITY`] frames behind; see
	/// [`Self::missed_notifications`].
	missed: AtomicU64,
	latest: PhantomData<T>,
	/// The `creator_id` that was originally used to create the parent
	/// subscription.
	creator_id: u32,
	/// The subscription `id` used to unsubscribe.
	id: SubscriptionId,
}

// Every field is unconditionally `Unpin` (`PhantomData<T>` carries no
// pinning through), so the handle can be polled without pin-projection.
impl<T: DeserializeOwned + WebSocketNotification> Unpin for Subscription<T> {}

impl<T: DeserializeOwned + WebSocketNotification> Clone for Subscription<T> {
	fn clone(&self) -> Self {
		// A fresh receiver at the live edge of the shared fan-out, mirroring
		// how an independent reader would join this subscription now.
		let receiver = self
			.shared
			.subscriptions
			.lock()
			.ok()
			.and_then(|subscriptions| subscriptions.get(&self.id).map(|sender| sender.subscribe()))
			.map(BroadcastStream::new)
			.unwrap_or_else(|| BroadcastStream::new(broadcast::channel(1).1));

		Self {
			receiver,
			shared: Arc::clone(&self.shared),
			unsubscribe_on_drop: Arc::clone(&self.unsubscribe_on_drop),
			missed: AtomicU64::new(0),
			latest: PhantomData,
			creator_id: self.creator_id,
			id: self.id,
		}
	}
}

impl<T: DeserializeOwned + WebSocketNotification> Subscription<T> {
	/// Adopt an existing subscription from a [`WebSocketProvider`], binding
	/// the request `id` that created it and the server's `subscription_id`.
	///
	/// Called by the subscribe methods on
	/// [`SolanaRpcClient`](crate::SolanaRpcClient); constructing one directly
	/// is only needed when driving a [`WebSocketProvider`] by hand.
	pub fn new(
		ws: &WebSocketProvider,
		id: u32,
		subscription_id: SubscriptionId,
	) -> Result<Self, ClientWebSocketError> {
		let (channel, receiver) = broadcast::channel(SUBSCRIPTION_CHANNEL_CAPACITY);
		ws.shared
			.subscriptions
			.lock()
			.map_err(|_| ClientWebSocketError::ConnectionError)?
			.insert(subscription_id, channel);

		Ok(Self {
			receiver: BroadcastStream::new(receiver),
			shared: Arc::clone(&ws.shared),
			unsubscribe_on_drop: Arc::new(AtomicBool::new(true)),
			missed: AtomicU64::new(0),
			latest: PhantomData,
			creator_id: id,
			id: subscription_id,
		})
	}

	/// Adopt a subscription from the receiver that
	/// [`WebSocketProvider::create_subscription`] registered before the
	/// subscribe request was sent.
	///
	/// This is the constructor the client's subscribe methods use; the
	/// receiver exists from before the request was sent, so a notification
	/// racing the handshake is still delivered.
	pub fn from_parts(
		ws: &WebSocketProvider,
		id: u32,
		subscription_id: SubscriptionId,
		receiver: broadcast::Receiver<Value>,
	) -> Self {
		Self {
			receiver: BroadcastStream::new(receiver),
			shared: Arc::clone(&ws.shared),
			unsubscribe_on_drop: Arc::new(AtomicBool::new(false)),
			missed: AtomicU64::new(0),
			latest: PhantomData,
			creator_id: id,
			id: subscription_id,
		}
	}

	/// Create a struct which will remove this subscription when the `run`
	/// method is called. This is useful since most uses of the subscription
	/// will consume the subscription. This can be invoked to store a way of
	/// removing the subscription even after it has been consumed in rust. You
	/// can also call [`Subscription::unsubscribe`].
	///
	/// ```
	/// use wasm_client_solana::LOCALNET;
	/// use wasm_client_solana::LogsSubscribeRequest;
	/// use wasm_client_solana::RpcTransactionLogsFilter;
	/// use wasm_client_solana::SolanaRpcClient;
	/// use wasm_client_solana::prelude::*;
	/// # use wasm_client_solana::ClientResult;
	///
	/// # async fn run() -> ClientResult<()> {
	/// let rpc = SolanaRpcClient::new(LOCALNET);
	/// let subscription = rpc
	/// 	.logs_subscribe(
	/// 		LogsSubscribeRequest::builder()
	/// 			.filter(RpcTransactionLogsFilter::AllWithVotes)
	/// 			.build(),
	/// 	)
	/// 	.await?;
	/// let unsubscription = subscription.get_unsubscription();
	/// let mut stream2 = subscription.clone().take(2);
	///
	/// while let Some(log_notification_request) = stream2.next().await {
	/// 	log::info!("The log notification {log_notification_request:#?}");
	/// }
	///
	/// // Can be called even though the `subscription` has been consumed.
	/// unsubscription.run().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_unsubscription(&self) -> Unsubscription {
		Unsubscription {
			method: T::UNSUBSCRIBE,
			shared: Arc::clone(&self.shared),
			unsubscribed: Arc::clone(&self.unsubscribe_on_drop),
			subscription_id: self.id,
		}
	}

	/// Unsubscribe from the websocket updates and wait for the node's ack.
	///
	/// Prefer this over letting the subscription drop when the ack matters;
	/// dropping sends the same request without waiting for it.
	pub async fn unsubscribe(&self) -> Result<(), ClientWebSocketError> {
		self.get_unsubscription().run().await?;

		Ok(())
	}

	/// The `id` originally used to create this subscription. It is also used
	/// to uniquely identify the unsubscription call.
	pub fn id(&self) -> u32 {
		self.creator_id
	}

	/// Get the `subscription_id` for this [`Subscription`].
	pub fn subscription_id(&self) -> SubscriptionId {
		self.id
	}

	/// Notifications this handle skipped because it fell more than
	/// [`SUBSCRIPTION_CHANNEL_CAPACITY`] frames behind the node.
	///
	/// A nonzero count means the consumer is too slow for the feed's rate:
	/// process the subscription on its own task, or clone it and split the
	/// work.
	pub fn missed_notifications(&self) -> u64 {
		self.missed.load(Ordering::Relaxed)
	}
}

impl<T: DeserializeOwned + WebSocketNotification> Drop for Subscription<T> {
	fn drop(&mut self) {
		if !self.unsubscribe_on_drop.load(Ordering::Relaxed) {
			let shared = Arc::clone(&self.shared);
			let method = T::UNSUBSCRIBE;
			let subscription_id = self.id;
			// No ack wait: the point is to stop the server sending frames
			// once nobody is listening, not to observe the confirmation.
			let unsubscribe = async move {
				if let Ok(mut subscriptions) = shared.subscriptions.lock() {
					subscriptions.remove(&subscription_id);
				}

				let request = ClientRequest::builder()
					.method(method)
					.params(serde_json::json!([subscription_id]))
					.build();
				if let Ok(request) = request.try_to_value() {
					let _ = shared.send(request).await;
				}
			};
			spawn_future(unsubscribe);
		}
	}
}

impl<T: DeserializeOwned + WebSocketNotification> Stream for Subscription<T> {
	type Item = SubscriptionResponse<T>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		let this = self.get_mut();

		loop {
			match this.receiver.poll_next_unpin(cx) {
				Poll::Ready(Some(Ok(value))) => {
					let Ok(json) = serde_json::from_value::<SubscriptionResponse<T>>(value) else {
						continue;
					};
					if json.method != T::NOTIFICATION {
						continue;
					}
					return Poll::Ready(Some(json));
				}
				// The handle fell behind by more than the channel capacity:
				// count the gap and keep streaming rather than ending the
				// subscription over it.
				Poll::Ready(Some(Err(missed))) => {
					let BroadcastStreamRecvError::Lagged(count) = missed;
					this.missed.fetch_add(count, Ordering::Relaxed);
				}
				Poll::Ready(None) => return Poll::Ready(None),
				Poll::Pending => return Poll::Pending,
			}
		}
	}
}

#[cfg(test)]
mod tests {

	use std::collections::HashMap;

	use assert2::check;
	use futures::StreamExt;
	use serde_json::Value;
	use tokio::sync::broadcast;

	use super::PendingRequest;
	use super::SUBSCRIPTION_CHANNEL_CAPACITY;
	use super::SubscriptionId;
	use super::envelope_subscription_id;
	use super::route;

	fn subscription_map() -> HashMap<SubscriptionId, broadcast::Sender<Value>> {
		HashMap::new()
	}

	/// A subscribe acknowledgement registers the fan-out under the
	/// server-assigned id before the caller is woken, so a notification
	/// racing the handshake is routed, not dropped.
	#[test]
	fn subscribe_ack_registers_channel_and_forwards_envelope() {
		let mut pending = HashMap::new();
		let mut subscriptions = subscription_map();
		let (channel, mut receiver) = broadcast::channel(SUBSCRIPTION_CHANNEL_CAPACITY);
		let (done, mut ack) = futures::channel::oneshot::channel();
		pending.insert(7, PendingRequest::Subscribe { done, channel });

		let ack_envelope: Value = serde_json::json!({ "jsonrpc": "2.0", "id": 7, "result": 42 });
		route(ack_envelope.clone(), &mut pending, &mut subscriptions);

		check!(pending.is_empty());
		check!(subscriptions.contains_key(&42));
		check!(ack.try_recv() == Ok(Some(ack_envelope)));

		// The very next frame for the new subscription is delivered even
		// though the caller has not been polled yet.
		let notification = serde_json::json!({
			"jsonrpc": "2.0",
			"method": "slotNotification",
			"params": { "result": { "slot": 1 }, "subscription": 42 },
		});
		route(notification.clone(), &mut pending, &mut subscriptions);
		check!(futures::executor::block_on(receiver.recv()) == Ok(notification));
	}

	/// A rejected subscribe carries an error instead of a result: no channel
	/// is registered and the caller's receiver closes with the envelope.
	#[test]
	fn rejected_subscribe_closes_channel() {
		let mut pending = HashMap::new();
		let mut subscriptions = subscription_map();
		let (channel, mut receiver) = broadcast::channel(SUBSCRIPTION_CHANNEL_CAPACITY);
		let (done, mut ack) = futures::channel::oneshot::channel();
		pending.insert(9, PendingRequest::Subscribe { done, channel });

		let error_envelope: Value = serde_json::json!({
			"jsonrpc": "2.0",
			"id": 9,
			"error": { "code": -32000, "message": "failed to subscribe" },
		});
		route(error_envelope.clone(), &mut pending, &mut subscriptions);

		check!(subscriptions.is_empty());
		check!(ack.try_recv() == Ok(Some(error_envelope)));
		check!(matches!(
			futures::executor::block_on(receiver.recv()),
			Err(broadcast::error::RecvError::Closed)
		));
	}

	/// Plain request acks (unsubscribes) are matched by id and removed from
	/// the pending map.
	#[test]
	fn plain_ack_is_delivered() {
		let mut pending = HashMap::new();
		let mut subscriptions = subscription_map();
		let (done, mut ack) = futures::channel::oneshot::channel();
		pending.insert(11, PendingRequest::Ack { done });

		let envelope: Value = serde_json::json!({ "jsonrpc": "2.0", "id": 11, "result": true });
		route(envelope.clone(), &mut pending, &mut subscriptions);

		check!(pending.is_empty());
		check!(ack.try_recv() == Ok(Some(envelope)));
	}

	/// Frames for unknown subscriptions or ids are ignored rather than
	/// failing the reader.
	#[test]
	fn unknown_frames_are_dropped() {
		let mut pending = HashMap::new();
		let mut subscriptions = subscription_map();

		let stray_ack: Value = serde_json::json!({ "jsonrpc": "2.0", "id": 404, "result": 1 });
		route(stray_ack, &mut pending, &mut subscriptions);

		let stray_notification: Value = serde_json::json!({
			"jsonrpc": "2.0",
			"method": "slotNotification",
			"params": { "result": { "slot": 2 }, "subscription": 404 },
		});
		route(stray_notification, &mut pending, &mut subscriptions);

		check!(pending.is_empty());
		check!(subscriptions.is_empty());
	}

	#[test]
	fn envelope_subscription_id_detection() {
		check!(envelope_subscription_id(&serde_json::json!({ "result": 5, "id": 1 })) == Some(5));
		check!(envelope_subscription_id(&serde_json::json!({ "id": 1 })) == None);
		check!(
			envelope_subscription_id(&serde_json::json!({
				"id": 1,
				"error": { "code": 1, "message": "no" },
			})) == None
		);
	}
}

#[cfg(feature = "ssr")]
mod websocket_provider_reqwest {
	use std::future::Future;
	use std::pin::Pin;
	use std::task::Context;
	use std::task::Poll;
	use std::task::ready;

	use futures::Sink;
	use futures::SinkExt;
	use futures::Stream;
	use futures::future::BoxFuture;
	use pin_project::pin_project;
	pub use reqwest_websocket::Error as WebSocketError;
	pub use reqwest_websocket::Message;
	use reqwest_websocket::WebSocket;
	use reqwest_websocket::websocket;
	use serde_json::Value;
	use typed_builder::TypedBuilder;

	use super::ToWebSocketValue;
	use crate::ClientWebSocketError;

	impl ToWebSocketValue for Message {
		fn to_websocket_value(&self) -> Result<Value, ClientWebSocketError> {
			let result = match self {
				Message::Text(string) => serde_json::from_str(string),
				Message::Binary(bytes) => serde_json::from_slice(bytes),
				_ => return Err(ClientWebSocketError::InvalidMessage),
			};

			result.map_err(|_| ClientWebSocketError::InvalidMessage)
		}
	}

	impl From<WebSocketError> for ClientWebSocketError {
		fn from(value: WebSocketError) -> Self {
			ClientWebSocketError::from(&value)
		}
	}

	impl From<&WebSocketError> for ClientWebSocketError {
		fn from(value: &WebSocketError) -> Self {
			match value {
				#[cfg(not(target_arch = "wasm32"))]
				WebSocketError::Handshake(_) => Self::ConnectionError,
				WebSocketError::Reqwest(_) => Self::ConnectionError,
				_ => Self::InvalidMessage,
			}
		}
	}

	type ReqwestResult = Result<WebSocket, WebSocketError>;

	/// A lazily-connected websocket that is both a [`Stream`] and a [`Sink`]
	/// of JSON [`Value`]s, for servers and native targets.
	///
	/// The connection handshake happens on first poll rather than at
	/// construction, so building the stream never blocks; frames that arrive
	/// as text or binary are parsed to JSON, and a synthetic
	/// `{"connected": true}` frame marks the moment the socket opens.
	#[derive(TypedBuilder)]
	#[pin_project]
	pub struct WebSocketStream {
		#[builder(setter(into))]
		url: String,
		#[pin]
		#[builder(default)]
		websocket: Option<WebSocket>,
		/// The in-flight handshake, latched to `None` the moment it
		/// completes. Both split halves share this stream, so without the
		/// latch a completed (typically failed) handshake gets polled a
		/// second time by the other half and panics inside
		/// `reqwest-websocket`.
		#[pin]
		#[builder(default)]
		initiator: Option<BoxFuture<'static, ReqwestResult>>,
	}

	impl WebSocketStream {
		/// Start connecting to `url`; poll the stream to drive the handshake.
		pub fn new(url: impl Into<String>) -> Self {
			let url = url.into();
			#[cfg(not(target_arch = "wasm32"))]
			let fut = websocket(url.clone());
			#[cfg(target_arch = "wasm32")]
			let fut = send_wrapper::SendWrapper::new(websocket(url.clone()));
			let boxed_future: BoxFuture<'static, ReqwestResult> = Box::pin(fut);

			WebSocketStream::builder()
				.initiator(Some(boxed_future))
				.url(url)
				.build()
		}
	}

	impl Stream for WebSocketStream {
		type Item = Result<Value, ClientWebSocketError>;

		fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
			let mut this = self.project();

			if let Some(websocket) = this.websocket.as_mut().as_pin_mut() {
				let Some(next) = ready!(websocket.poll_next(cx)) else {
					return Poll::Ready(None);
				};

				return Poll::Ready(Some(next.to_websocket_value()));
			}

			// A completed handshake is never polled again: surface its
			// failure once as an error frame (rather than a silent end of
			// stream), then report the stream as ended on every later poll.
			let Some(initiator) = this.initiator.as_mut().as_pin_mut() else {
				return Poll::Ready(None);
			};
			let result = ready!(initiator.poll(cx));
			this.initiator.as_mut().set(None);

			let Ok(websocket) = result else {
				return Poll::Ready(Some(Err(ClientWebSocketError::ConnectionError)));
			};

			this.websocket.set(Some(websocket));

			Poll::Ready(Some(Ok(serde_json::json!({ "connected": true }))))
		}
	}

	impl Sink<Value> for WebSocketStream {
		type Error = ClientWebSocketError;

		fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
			let mut this = self.project();

			if let Some(mut websocket) = this.websocket.as_mut().as_pin_mut() {
				return websocket.poll_ready_unpin(cx).map_err(Into::into);
			}

			let Some(initiator) = this.initiator.as_mut().as_pin_mut() else {
				return Poll::Ready(Err(ClientWebSocketError::ConnectionError));
			};
			let result = ready!(initiator.poll(cx));
			this.initiator.as_mut().set(None);

			if let Ok(mut websocket) = result {
				let poll_result = websocket.poll_ready_unpin(cx).map_err(Into::into);
				this.websocket.set(Some(websocket));
				return poll_result;
			}

			Poll::Ready(Err(ClientWebSocketError::ConnectionError))
		}

		fn start_send(mut self: Pin<&mut Self>, item: Value) -> Result<(), Self::Error> {
			let Some(websocket) = self.websocket.as_mut() else {
				return Err(ClientWebSocketError::ConnectionError);
			};

			let text =
				Message::text_from_json(&item).map_err(|_| ClientWebSocketError::InvalidMessage)?;
			websocket.start_send_unpin(text).map_err(Into::into)
		}

		fn poll_flush(
			mut self: Pin<&mut Self>,
			cx: &mut Context<'_>,
		) -> Poll<Result<(), Self::Error>> {
			let Some(websocket) = self.websocket.as_mut() else {
				return Poll::Pending;
			};

			websocket.poll_flush_unpin(cx).map_err(Into::into)
		}

		fn poll_close(
			mut self: Pin<&mut Self>,
			cx: &mut Context<'_>,
		) -> Poll<Result<(), Self::Error>> {
			let Some(websocket) = self.websocket.as_mut() else {
				return Poll::Pending;
			};

			websocket.poll_close_unpin(cx).map_err(Into::into)
		}
	}

	#[cfg(test)]
	mod tests {

		use assert2::check;
		use futures::StreamExt;

		use super::WebSocketStream;
		use crate::errors::ClientWebSocketError;

		/// A refused connection must surface as a single error frame followed
		/// by a permanent end of stream — never a panic from re-polling the
		/// completed handshake, and never a silent empty stream.
		#[tokio::test]
		async fn failed_handshake_errors_once_then_ends() {
			let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
			let port = socket.local_addr().unwrap().port();
			drop(socket);

			let mut stream = WebSocketStream::new(format!("ws://127.0.0.1:{port}"));

			let first = stream.next().await;
			check!(matches!(
				first,
				Some(Err(ClientWebSocketError::ConnectionError))
			));

			// The latched handshake keeps the stream terminal without
			// touching the completed future again.
			let second = stream.next().await;
			check!(second.is_none());
			let third = stream.next().await;
			check!(third.is_none());
		}
	}
}

#[cfg(not(feature = "ssr"))]
mod websocket_provider_wasm {
	use std::pin::Pin;
	use std::task::Context;
	use std::task::Poll;
	use std::task::ready;

	use futures::Sink;
	use futures::SinkExt;
	use futures::Stream;
	use gloo_net::websocket::Message;
	use gloo_net::websocket::futures::WebSocket;
	use pin_project::pin_project;
	use serde_json::Value;
	use typed_builder::TypedBuilder;
	use wasm_bindgen::UnwrapThrowExt;

	use super::ToWebSocketValue;
	use crate::ClientWebSocketError;

	/// An eagerly-connected websocket that is both a [`Stream`] and a [`Sink`]
	/// of JSON [`Value`]s, backed by gloo for browser targets.
	///
	/// Unlike the reqwest variant the connection is opened at construction —
	/// in a browser the JS API is callback-based, so there is no handshake
	/// future to defer — which is why the browser transports must run inside
	/// `SendWrapper`.
	#[derive(TypedBuilder)]
	#[pin_project]
	pub struct WebSocketStream {
		#[builder(setter(into))]
		url: String,
		#[pin]
		websocket: WebSocket,
	}

	impl WebSocketStream {
		/// Open the connection to `url`; panics if the browser refuses it,
		/// matching the fail-fast behavior of the JS websocket constructor.
		pub fn new(url: &str) -> Self {
			Self::builder()
				.url(url)
				.websocket(WebSocket::open(url).unwrap_throw())
				.build()
		}
	}

	impl Stream for WebSocketStream {
		type Item = Result<Value, ClientWebSocketError>;

		fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
			let mut this = self.project();

			let Some(result) = ready!(this.websocket.as_mut().poll_next(cx)) else {
				return Poll::Ready(None);
			};

			Poll::Ready(Some(result.to_websocket_value()))
		}
	}

	impl Sink<Value> for WebSocketStream {
		type Error = ClientWebSocketError;

		fn poll_ready(
			mut self: Pin<&mut Self>,
			cx: &mut Context<'_>,
		) -> Poll<Result<(), Self::Error>> {
			self.websocket.poll_ready_unpin(cx).map_err(Into::into)
		}

		fn start_send(mut self: Pin<&mut Self>, item: Value) -> Result<(), Self::Error> {
			let string =
				serde_json::to_string(&item).map_err(|_| ClientWebSocketError::InvalidMessage)?;
			let text = Message::Text(string);

			self.websocket.start_send_unpin(text).map_err(Into::into)
		}

		fn poll_flush(
			mut self: Pin<&mut Self>,
			cx: &mut Context<'_>,
		) -> Poll<Result<(), Self::Error>> {
			self.websocket.poll_flush_unpin(cx).map_err(Into::into)
		}

		fn poll_close(
			mut self: Pin<&mut Self>,
			cx: &mut Context<'_>,
		) -> Poll<Result<(), Self::Error>> {
			self.websocket.poll_close_unpin(cx).map_err(Into::into)
		}
	}

	impl ToWebSocketValue for Message {
		fn to_websocket_value(&self) -> Result<Value, ClientWebSocketError> {
			let result = match self {
				Message::Text(string) => serde_json::from_str(string),
				Message::Bytes(bytes) => serde_json::from_slice(bytes),
			};

			result.map_err(|_| ClientWebSocketError::InvalidMessage)
		}
	}
}
