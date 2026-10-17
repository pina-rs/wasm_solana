//! Callback-driven subscriptions: the push-based alternative to polling a
//! [`Subscription`](crate::Subscription) stream.
//!
//! A stream subscription hands the caller a [`Stream`] they must drive
//! themselves — usually inside a UI loop or a spawned task. A callback
//! subscription inverts the control flow: the caller supplies a closure and
//! the crate drives the stream, invoking the closure once per notification on
//! the target's runtime (`wasm-bindgen-futures` in the browser, tokio under
//! `ssr`). Both styles share the same websocket, the same wire protocol and
//! the same notification types; pick whichever fits the surrounding code.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use futures::Stream;
use futures::StreamExt;
use futures::channel::oneshot;
use futures::future::AbortHandle;
use futures::future::Abortable;
use futures::future::Fuse;
use futures::future::FutureExt;
use futures::future::Pending;
use futures::future::pending;
use serde::de::DeserializeOwned;

use crate::ClientWebSocketError;
use crate::Subscription;
use crate::SubscriptionId;
use crate::WebSocketNotification;

cfg_if::cfg_if! {
	if #[cfg(feature = "ssr")] {
		/// The bounds a notification callback must satisfy to be driven by the
		/// target runtime.
		///
		/// Under `ssr` the pump runs on `tokio`, so the closure must be `Send`;
		/// in the browser there is no such requirement. The bound is a
		/// blanket-implemented trait, so plain closures satisfy it without any
		/// ceremony.
		pub trait SubscriptionCallback<T>: FnMut(T) + Send + 'static {}
		impl<F, T> SubscriptionCallback<T> for F where F: FnMut(T) + Send + 'static {}
	} else {
		/// The bounds a notification callback must satisfy to be driven by the
		/// target runtime.
		///
		/// In the browser the pump runs on `wasm-bindgen-futures`, which has no
		/// `Send` requirement, so any `FnMut` closure works — including ones
		/// capturing `Rc`, `RefCell` or raw JavaScript handles.
		pub trait SubscriptionCallback<T>: FnMut(T) + 'static {}
		impl<F, T> SubscriptionCallback<T> for F where F: FnMut(T) + 'static {}
	}
}

/// The cancel arm of every pump: only ever resolves through
/// [`AbortHandle::abort`], because `pending` never completes and dropping an
/// `AbortHandle` does not abort.
type PumpCancel = Fuse<Abortable<Pending<()>>>;

/// Create the cancel arm and its abort handle.
fn cancel_pair() -> (AbortHandle, PumpCancel) {
	let (abort, registration) = AbortHandle::new_pair();
	let cancel = Abortable::new(pending::<()>(), registration).fuse();
	(abort, cancel)
}

/// Drive `stream`, invoking `callback` once per item, until `cancel` resolves
/// or the stream ends. Returns whether the cancel arm ended the pump, and
/// hands the stream back so its owner can use it after the pump stops.
///
/// Selecting against `cancel` (rather than checking a flag between items)
/// means an abort stops the pump even while it is parked waiting for an item
/// that may never arrive — the shape of a quiet websocket.
async fn run_pump<S, F, T>(mut stream: S, mut callback: F, mut cancel: PumpCancel) -> (bool, S)
where
	S: Stream<Item = T> + Unpin,
	F: FnMut(T),
{
	loop {
		futures::select! {
			_ = &mut cancel => return (true, stream),
			item = stream.next().fuse() => {
				let Some(item) = item else {
					return (false, stream);
				};

				callback(item);
			}
		}
	}
}

/// Schedule the callback pump on the runtime available to the target.
///
/// Mirrors [`spawn_local`](crate::spawn_local) but uses `tokio::spawn` under
/// `ssr`, so the pump works on any tokio runtime — multi-thread or
/// current-thread — without a `LocalSet`. Panics outside a tokio runtime,
/// like every other `tokio::spawn` call.
#[cfg(feature = "ssr")]
fn spawn_pump<F: Future<Output = ()> + Send + 'static>(pump: F) {
	tokio::task::spawn(pump);
}

/// Schedule the callback pump on the browser's microtask queue; wasm futures
/// need not be `Send`, so callbacks may capture `Rc` or JavaScript handles.
#[cfg(not(feature = "ssr"))]
fn spawn_pump<F: Future<Output = ()> + 'static>(pump: F) {
	wasm_bindgen_futures::spawn_local(pump);
}

/// A subscription driven by a callback instead of a polled stream.
///
/// Created by the `on_*` methods on
/// [`SolanaRpcClient`](crate::SolanaRpcClient) — `on_account_change`,
/// `on_logs`, `on_slot_change`, and friends — or generically through
/// [`subscribe_with_callback`](crate::SolanaRpcClient::subscribe_with_callback).
/// The pump task drives the shared websocket, invoking the callback once per
/// matching notification.
///
/// # Lifetime
///
/// - [`CallbackSubscription::unsubscribe`] stops the callback, unsubscribes on
///   the server and waits for the acknowledgement.
/// - Dropping the handle stops the callback *and* asks the server to
///   unsubscribe in the background — the fire-and-forget teardown the stream
///   flavor cannot offer, because a `Drop` body cannot await. The request is
///   sent from the pump task as it exits; nothing waits for its ack.
/// - [`CallbackSubscription::detach`] keeps the pump running after the handle
///   is dropped, for subscriptions that should live as long as the connection.
///
/// ```no_run
/// # use wasm_client_solana::SolanaRpcClient;
/// # use wasm_client_solana::SlotSubscribeRequest;
/// # async fn run(rpc: SolanaRpcClient) -> wasm_client_solana::ClientResult<()> {
/// let slots = rpc
/// 	.on_slot_change(SlotSubscribeRequest::builder().build(), |slot| {
/// 		log::info!("new slot {}", slot.slot);
/// 	})
/// 	.await?;
///
/// // ... later, when the UI no longer cares:
/// slots.unsubscribe().await?;
/// # Ok(())
/// # }
/// ```
/// Debug-printable for `Result::unwrap_err` in tests and diagnostics.
#[derive(Debug)]
pub struct CallbackSubscription {
	/// The request `id` that created the subscription.
	creator_id: u32,
	/// The server-assigned subscription id.
	subscription_id: SubscriptionId,
	/// Aborts the pump; set from `unsubscribe` and `Drop` (unless detached).
	abort: AbortHandle,
	/// Set by the pump when it exits for any reason, so `is_stopped` reflects
	/// a dead socket as well as an explicit stop.
	finished: Arc<AtomicBool>,
	/// Set by `detach`, so `Drop` leaves the pump running and the pump skips
	/// the parting unsubscription.
	detached: Arc<AtomicBool>,
	/// The pump's report of its parting unsubscription, awaited by
	/// `unsubscribe`.
	done: Option<oneshot::Receiver<Result<(), ClientWebSocketError>>>,
}

impl CallbackSubscription {
	/// Drive `subscription` on the target runtime, invoking `callback` once
	/// per notification payload.
	///
	/// The payload must be `Send + Sync + 'static` because the pump owns it
	/// across awaits on a multithreaded runtime; every notification type in
	/// this crate satisfies that.
	pub fn new<T, F>(subscription: Subscription<T>, mut callback: F) -> Self
	where
		T: DeserializeOwned + WebSocketNotification + Send + Sync + 'static,
		F: SubscriptionCallback<T>,
	{
		let (abort, cancel) = cancel_pair();
		let finished = Arc::new(AtomicBool::new(false));
		let detached = Arc::new(AtomicBool::new(false));
		let (done_sender, done) = oneshot::channel();
		let creator_id = subscription.id();
		let subscription_id = subscription.subscription_id();

		let pump_finished = Arc::clone(&finished);
		let pump_detached = Arc::clone(&detached);
		let payload_callback = move |notification: crate::SubscriptionResponse<T>| {
			callback(notification.params.result);
		};
		let pump = async move {
			let (cancelled, subscription) = run_pump(subscription, payload_callback, cancel).await;

			// A stopped pump unsubscribes from its own, fully drained fork —
			// positioned at the live edge, so the ack wait replays nothing —
			// unless the handle was detached.
			let outcome = if cancelled && !pump_detached.load(Ordering::Acquire) {
				subscription.unsubscribe().await
			} else {
				Ok(())
			};

			let _ = done_sender.send(outcome);
			pump_finished.store(true, Ordering::Release);
		};

		spawn_pump(pump);

		Self {
			creator_id,
			subscription_id,
			abort,
			finished,
			detached,
			done: Some(done),
		}
	}

	/// The request `id` that originally created this subscription.
	pub fn id(&self) -> u32 {
		self.creator_id
	}

	/// The server-assigned identifier of this subscription.
	pub fn subscription_id(&self) -> SubscriptionId {
		self.subscription_id
	}

	/// Whether the pump has stopped — through `unsubscribe`, a drop, or a
	/// dead websocket. A detached handle reports `false` until the socket
	/// itself closes.
	pub fn is_stopped(&self) -> bool {
		self.abort.is_aborted() || self.finished.load(Ordering::Acquire)
	}

	/// Keep the pump running after this handle is dropped.
	///
	/// The callback keeps firing until the websocket ends and no
	/// unsubscription is ever sent, so only detach when the subscription
	/// should live as long as the connection.
	pub fn detach(self) {
		self.detached.store(true, Ordering::Release);
	}

	/// Stop the callback, unsubscribe on the server and wait for its ack.
	///
	/// The pump sends the unsubscription from its own fork — positioned at
	/// the live edge — and this returns the outcome of that request. When the
	/// websocket already ended there is nothing to unsubscribe from and the
	/// result is `Ok(())`.
	pub async fn unsubscribe(mut self) -> Result<(), ClientWebSocketError> {
		self.abort.abort();

		let done = self.done.take();

		match done.expect("unsubscribe can only be called once").await {
			// The pump reports the unsubscription outcome; a dropped sender
			// means the pump ended without sending, which is itself success.
			Ok(outcome) => outcome,
			Err(_) => Ok(()),
		}
	}
}

impl Drop for CallbackSubscription {
	fn drop(&mut self) {
		if !self.detached.load(Ordering::Acquire) {
			// The pump wakes, stops invoking the callback and unsubscribes in
			// the background.
			self.abort.abort();
		}
	}
}

#[cfg(all(test, feature = "ssr", not(target_arch = "wasm32")))]
mod tests {

	use std::sync::Arc;
	use std::sync::Mutex;

	use assert2::check;
	use futures::stream;

	use super::cancel_pair;
	use super::run_pump;

	/// An abort must stop the pump while it is parked on a stream that never
	/// yields — the shape of unsubscribing from a quiet websocket — and
	/// report the cancel as the reason.
	#[tokio::test]
	async fn abort_stops_a_parked_pump() {
		let (abort, cancel) = cancel_pair();
		let pump = run_pump(stream::pending::<u32>(), |_: u32| {}, cancel);

		let handle = tokio::task::spawn(pump);
		// Give the pump a chance to park; the abort must still wake it.
		tokio::time::sleep(std::time::Duration::from_millis(10)).await;
		abort.abort();
		let (cancelled, _) = handle.await.unwrap();

		check!(cancelled);
	}

	/// The callback receives every item in order, and a bounded stream ends
	/// the pump on its own — reported as a natural end, not a cancel.
	#[tokio::test]
	async fn pump_forwards_every_item_then_ends() {
		let seen = Arc::new(Mutex::new(Vec::new()));
		let sink = Arc::clone(&seen);
		let (abort, cancel) = cancel_pair();

		let (cancelled, _) = run_pump(
			stream::iter(0_u32..4),
			move |value: u32| sink.lock().unwrap().push(value),
			cancel,
		)
		.await;

		check!(*seen.lock().unwrap() == vec![0, 1, 2, 3]);
		check!(!cancelled);
		abort.abort();
	}
}
