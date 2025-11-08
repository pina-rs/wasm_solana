use std::hash::Hash;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use fork_stream::Forked;
use fork_stream::StreamExt as _;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use futures::future;
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

pub trait ToWebSocketValue {
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

#[derive(Clone, derive_more::Debug)]
pub struct WebSocketProvider {
	/// The websocket url.
	url: String,
	/// The client ID which identifies current client ID.
	id: Arc<std::sync::Mutex<u32>>,
	#[debug(skip)]
	sender: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	#[debug(skip)]
	receiver: Forked<SplitStream<WebSocketStream>>,
}

impl WebSocketProvider {
	pub fn new(url: impl Into<String>) -> Self {
		let url = get_ws_url(url);
		let stream = WebSocketStream::new(&url);
		let (sink, stream) = stream.split();
		let receiver = stream.fork();
		let sender = Arc::new(Mutex::new(sink));

		Self {
			url,
			// start with 1000 since the default id used for http methods is 0
			id: Arc::new(std::sync::Mutex::new(1000)),
			sender,
			receiver,
		}
	}

	pub fn url(&self) -> &str {
		&self.url
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

		let mut stream = self.receiver.clone().filter_map(|value| {
			let Ok(value) = value else {
				return future::ready(None);
			};

			future::ready(
				serde_json::from_value::<SubscriptionResult>(value)
					.ok()
					.filter(|value| value.id == id),
			)
		});

		let Some(response) = stream.next().await else {
			return Err(ClientWebSocketError::Subscription);
		};

		Ok((id, response.result))
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
	/// The shared receiver for websocket messages.
	pub(crate) receiver: Forked<SplitStream<WebSocketStream>>,
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

		let mut stream = self.receiver.filter_map(|value| {
			let Ok(value) = value else {
				return future::ready(None);
			};

			future::ready(
				serde_json::from_value::<UnsubscriptionResult>(value)
					.ok()
					.filter(|value| value.id == self.id),
			)
		});

		let Some(_) = stream.next().await else {
			return Err(ClientWebSocketError::Unsubscription);
		};

		Ok(())
	}
}

/// A [`Subscription`] is used to managed a solana websocket rpc method.
#[pin_project]
#[derive(Clone, TypedBuilder)]
pub struct Subscription<T: DeserializeOwned + WebSocketNotification> {
	/// The shared receiver for receiving messages.
	#[pin]
	pub(crate) receiver: Forked<SplitStream<WebSocketStream>>,
	/// The shared sink for pushing messages into the websocket stream.
	pub(crate) sender: Arc<Mutex<SplitSink<WebSocketStream, Value>>>,
	#[builder(default)]
	pub(crate) latest: PhantomData<T>,
	/// The `id` that was originally used to create the parent subscription.
	pub(crate) creator_id: u32,
	/// The `subscription_id` used to unsubscribe.
	pub(crate) id: SubscriptionId,
	// pub(crate) unsubscription: Unsubscription,
}

impl<T: DeserializeOwned + WebSocketNotification> Subscription<T> {
	pub fn new(ws: &WebSocketProvider, id: u32, subscription_id: SubscriptionId) -> Self {
		Self::builder()
			.receiver(ws.receiver.clone())
			.sender(ws.sender.clone())
			.creator_id(id)
			.id(subscription_id)
			.build()
	}

	/// Create a struct which will remove this subscription when the `run`
	/// method is called. This is useful since most uses of the subscription
	/// will consume the subscription. This can be invoked to store a way of
	/// removing the subscription even after it has been consumed in rust. You
	/// can also call [`Subscription::unsubscribe`].
	///
	/// ```
	/// use solana_pubkey::Pubkey;
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
			.receiver(self.receiver.clone())
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
		let subscription_id = self.get_unsubscription().subscription_id;
		let mut this = self.project();

		let Some(result) = ready!(this.receiver.as_mut().poll_next(cx)) else {
			return Poll::Ready(None);
		};

		let Ok(value) = result else {
			return Poll::Pending;
		};

		let Some(json) = serde_json::from_value::<SubscriptionResponse<T>>(value).ok() else {
			return Poll::Pending;
		};

		if json.method != T::NOTIFICATION || json.params.subscription != subscription_id {
			return Poll::Pending;
		}

		Poll::Ready(Some(json))
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
		#[builder(default)]
		ended: bool,
	}

	impl WebSocketStream {
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
					this.ended = &mut true;
					return Poll::Ready(None);
				};

				return Poll::Ready(Some(next.to_websocket_value()));
			}

			let initiator = this.initiator.as_mut();
			let result = ready!(initiator.poll(cx));

			let Ok(websocket) = result else {
				this.ended = &mut true;
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

	#[derive(TypedBuilder)]
	#[pin_project]
	pub struct WebSocketStream {
		#[builder(setter(into))]
		url: String,
		#[pin]
		websocket: WebSocket,
	}

	impl WebSocketStream {
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
