use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use fork_stream::Forked;
use fork_stream::StreamExt as _;
use fork_stream::Weak;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use futures::lock::Mutex;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use pin_project::pin_project;
use serde::de::DeserializeOwned;
use serde_json::Value;
use typed_builder::TypedBuilder;

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

/// A cloneable handle to the live edge of the shared websocket buffer.
/// `fork_stream::Weak` itself is not `Clone`, so it is shared behind an
/// `Arc`; upgrading yields a fork positioned at the next frame to arrive.
type LiveEdge = Arc<Weak<SplitStream<WebSocketStream>>>;

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

/// A connection to a Solana node's pubsub endpoint.
///
/// Cloning is cheap and every clone shares one socket: the sink is guarded by
/// a mutex and the receiver is a [`Forked`] stream that each clone reads
/// independently while the underlying frames are buffered once. That sharing
/// is what lets many [`Subscription`]s — plus their unsubscriptions — coexist
/// on a single websocket, which browsers limit the number of.
#[derive(Clone, derive_more::Debug)]
pub struct WebSocketProvider {
	/// The websocket url.
	url: String,
	/// The client ID which identifies current client ID.
	id: Arc<std::sync::Mutex<u32>>,
	#[debug(skip)]
	sender: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	/// The root fork of the shared buffer. It is never read: it only keeps
	/// the shared stream alive for as long as the provider exists, so that
	/// [`Self::live_edge`] upgrades cannot fail while the provider lives.
	#[allow(dead_code)]
	#[debug(skip)]
	receiver: Forked<SplitStream<WebSocketStream>>,
	/// A [`Weak`] handle used to hand out forks at the *live edge* of the
	/// shared buffer. Cloning the root fork instead would start every
	/// subscription at the buffer's oldest entry, replaying the socket's
	/// entire history before reaching live traffic — subscription setup cost
	/// that grows linearly with the connection's age (see the
	/// `subscription/ack_wait` benchmarks).
	#[debug(skip)]
	live_edge: LiveEdge,
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
		let receiver = stream.fork();
		let live_edge = Arc::new(receiver.downgrade());
		let sender = Arc::new(Mutex::new(sink));

		Self {
			url,
			// start with 1000 since the default id used for http methods is 0
			id: Arc::new(std::sync::Mutex::new(1000)),
			sender,
			receiver,
			live_edge,
		}
	}

	/// The websocket endpoint URL, after the http-to-ws rewrite done at
	/// construction.
	pub fn url(&self) -> &str {
		&self.url
	}

	/// A fork positioned at the live edge of the shared buffer — the next
	/// frame to arrive — so ack waits and new subscriptions never replay the
	/// socket's history.
	///
	/// Fails only when the shared buffer is gone, which cannot happen while
	/// this provider (and its root fork) is alive.
	fn live_fork(&self) -> Result<Forked<SplitStream<WebSocketStream>>, ClientWebSocketError> {
		self.live_edge
			.upgrade()
			.ok_or(ClientWebSocketError::ConnectionError)
	}

	/// Create a subscription and return the `id` used to create the
	/// subscription and `subscription_id` once a response is received.
	pub async fn create_subscription<T: WebSocketMethod>(
		&self,
		params: T,
	) -> Result<(u32, SubscriptionId), ClientWebSocketError> {
		let id = self.next_id()?;
		let request = ClientRequest::builder()
			.method(T::SUBSCRIBE)
			.params(params)
			.id(id)
			.build()
			.try_to_value()?;

		// immediately drop the lock at the end of this block
		{
			let mut lock = self.sender.lock().await;
			lock.send(request)
				.await
				.map_err(|_| ClientWebSocketError::MessageSendError)?;
		}

		// Wait for the ack with a manual re-poll loop rather than
		// `filter_map`: the combinator returns `Pending` after consuming a
		// buffered frame that does not match, and `Forked` only registers the
		// caller's waker with the websocket once its buffer runs dry — so a
		// `Pending` returned while replayed frames remain would park this
		// future forever. Re-polling drains the buffer and re-arms the socket
		// waker in the same poll.
		let mut stream = self.live_fork()?;

		loop {
			let Some(result) = stream.next().await else {
				return Err(ClientWebSocketError::Subscription);
			};

			let Ok(value) = result else {
				continue;
			};

			if let Ok(response) = serde_json::from_value::<SubscriptionResult>(value)
				&& response.id == id
			{
				return Ok((id, response.result));
			}
		}
	}

	fn next_id(&self) -> Result<u32, ClientWebSocketError> {
		let mut id_guard = self
			.id
			.lock()
			.map_err(|_| ClientWebSocketError::ConnectionError)?;
		let current_id = *id_guard;
		*id_guard += 1;

		Ok(current_id)
	}
}

/// Created from a [`Subscription`] to send a message to unsubscribe.
#[derive(Clone, TypedBuilder)]
pub struct Unsubscription {
	/// The name of the method used to unsubscribe.
	pub(crate) method: &'static str,
	/// The shared sink for pushing messages into the websocket stream.
	pub(crate) sender: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	/// Live-edge handle for the shared receiver; upgraded when `run` sends
	/// the request, so the ack wait starts at the traffic that follows it.
	pub(crate) receiver: LiveEdge,
	/// The `id` that was originally used to create the parent subscription.
	pub(crate) id: u32,
	/// The `subscription_id` used to unsubscribe.
	pub(crate) subscription_id: SubscriptionId,
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
	/// Consumes `self` because the shared receiver is moved into the wait
	/// loop. Errors if the request cannot be written or the connection ends
	/// before the ack arrives.
	pub async fn run(self) -> Result<(), ClientWebSocketError> {
		let request = ClientRequest::builder()
			.id(self.id)
			.method(self.method)
			.params(serde_json::json!([self.subscription_id]))
			.build()
			.try_to_value()?;

		// drop the lock immediately after this block
		{
			let mut lock = self.sender.lock().await;
			lock.send(request)
				.await
				.map_err(|_| ClientWebSocketError::ConnectionError)?;
		}

		// Same re-poll loop as `create_subscription`: `filter_map` would park
		// this future whenever it consumed a replayed frame that does not
		// match, because `Forked` only arms the socket waker once its buffer
		// is empty.
		let mut stream = self
			.receiver
			.upgrade()
			.ok_or(ClientWebSocketError::ConnectionError)?;

		loop {
			let Some(result) = stream.next().await else {
				return Err(ClientWebSocketError::Unsubscription);
			};

			let Ok(value) = result else {
				continue;
			};

			if let Ok(response) = serde_json::from_value::<UnsubscriptionResult>(value)
				&& response.id == self.id
			{
				return Ok(());
			}
		}
	}
}

/// A [`Subscription`] is used to managed a solana websocket rpc method.
#[pin_project]
#[derive(Clone, TypedBuilder)]
pub struct Subscription<T: DeserializeOwned + WebSocketNotification> {
	/// The shared receiver for receiving messages, forked at the live edge
	/// when the subscription was created.
	#[pin]
	pub(crate) receiver: Forked<SplitStream<WebSocketStream>>,
	/// Live-edge handle handed to [`Unsubscription`] so its ack wait skips
	/// the frames this subscription has already consumed.
	pub(crate) live_edge: LiveEdge,
	/// The shared sink for pushing messages into the websocket stream.
	pub(crate) sender: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	#[builder(default)]
	pub(crate) latest: PhantomData<T>,
	/// The `creator_id` that was originally used to create the parent
	/// subscription.
	pub(crate) creator_id: u32,
	/// The subscription `id` used to unsubscribe.
	pub(crate) id: SubscriptionId,
	// pub(crate) unsubscription: Unsubscription,
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
		Ok(Self::builder()
			.receiver(ws.live_fork()?)
			.live_edge(ws.live_edge.clone())
			.sender(ws.sender.clone())
			.creator_id(id)
			.id(subscription_id)
			.build())
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
		Unsubscription::builder()
			.method(T::UNSUBSCRIBE)
			.sender(self.sender.clone())
			.receiver(self.live_edge.clone())
			.id(self.creator_id)
			.subscription_id(self.id)
			.build()
	}

	/// This must be called to unsubscribe from the websocket updates. It would
	/// be nice if there was a way to automatically do this on `Drop`. However,
	/// I'm not sure how to make async updates on drop. `spawn_local` was
	/// failing.
	pub async fn unsubscribe(&self) -> Result<(), ClientWebSocketError> {
		self.get_unsubscription().run().await?;

		Ok(())
	}

	/// The `id` originally used to create this subscription. It is also used to
	/// uniquely identify the unsubscription call.
	pub fn id(&self) -> u32 {
		self.creator_id
	}

	/// Get the `subscription_id` for this [`Subscription`].
	pub fn subscription_id(&self) -> SubscriptionId {
		self.id
	}
}

impl<T: DeserializeOwned + WebSocketNotification> Stream for Subscription<T> {
	type Item = SubscriptionResponse<T>;

	fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
		let subscription_id = self.id;
		let mut this = self.project();

		// Loop over frames until one belongs to this subscription. Returning
		// `Pending` after consuming a buffered frame would drop the task's
		// wakeup: `Forked` only registers the caller's waker with the
		// websocket when its buffer runs dry, so a `Pending` returned while
		// buffered frames remain would park this task forever. Re-polling
		// drains the buffer and re-arms the socket waker in the same poll.
		loop {
			let Some(result) = ready!(this.receiver.as_mut().poll_next(cx)) else {
				return Poll::Ready(None);
			};

			let Ok(value) = result else {
				continue;
			};

			let Ok(json) = serde_json::from_value::<SubscriptionResponse<T>>(value) else {
				continue;
			};

			if json.method == T::NOTIFICATION && json.params.subscription == subscription_id {
				return Poll::Ready(Some(json));
			}
		}
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
		#[pin]
		initiator: BoxFuture<'static, ReqwestResult>,
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
				.initiator(boxed_future)
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

			let initiator = this.initiator.as_mut();
			let result = ready!(initiator.poll(cx));

			let Ok(websocket) = result else {
				return Poll::Ready(None);
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

			let initiator = this.initiator.as_mut();
			let result = ready!(initiator.poll(cx));

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
