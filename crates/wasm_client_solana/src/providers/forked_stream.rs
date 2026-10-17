//! A fork of [`fork_stream`] 0.1.0 (MIT OR Apache-2.0) with corrected wakeup
//! semantics, vendored so the published crate can carry the fix.
//!
//! The published crate registers a single waker with the source stream — the
//! last poller's — and wakes one buffered waiter per pushed item. When that
//! last poller's task finishes (a confirmation's unsubscription ack, for
//! example) the source keeps its dead waker, and parked forks queued behind
//! stale wakers never hear about new frames: a subscription can starve even
//! though the socket is healthy.
//!
//! This fork keeps the buffer and offset model but registers every parked
//! fork with the source through one shared waker, and wakes *all* parked
//! forks whenever an item lands. Stale wakers cost a spurious no-op wake;
//! they can no longer hide a live one.
//!
//! [`fork_stream`]: https://docs.rs/fork_stream/0.1.0

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;

use futures::Stream;
use futures::StreamExt as _;
use futures::stream::Fuse;

/// An extension trait that turns any clone-item stream into a forkable one.
pub trait ForkStreamExt: Stream {
	/// Wrap `self` so clones can each read every item.
	fn fork(self) -> Forked<Self>
	where
		Self: Sized,
		Self::Item: Clone,
	{
		ForkedInner::init(self)
	}
}

impl<S: Stream> ForkStreamExt for S {}

/// The wakers of every fork currently parked waiting for progress, plus the
/// one waker handed to the source stream: waking it wakes every parked fork.
#[derive(Default)]
struct WakerRegistry {
	parked: Vec<Waker>,
}

impl WakerRegistry {
	/// Register `waker` as parked, replacing any entry that would wake the
	/// same task.
	fn park(&mut self, waker: &Waker) {
		if let Some(existing) = self.parked.iter_mut().find(|w| w.will_wake(waker)) {
			existing.clone_from(waker);

			return;
		}

		self.parked.push(waker.clone());
	}

	/// Wake every parked fork. Called whenever an item lands, the source
	/// ends, or the source itself reports readiness.
	fn wake_all(&mut self) {
		for waker in self.parked.drain(..) {
			waker.wake();
		}
	}
}

/// The `Waker` handed to the source stream: wakes the whole registry.
struct SourceWaker(Arc<Mutex<WakerRegistry>>);

impl Wake for SourceWaker {
	fn wake(self: Arc<Self>) {
		Self::wake_by_ref(&self);
	}

	fn wake_by_ref(self: &Arc<Self>) {
		self.0.lock().unwrap().wake_all();
	}
}

#[pin_project::pin_project]
struct ForkedInner<S: Stream>
where
	S::Item: Clone,
{
	#[pin]
	source: Fuse<S>,
	buffer: Buffer<S::Item>,
	wakers: Arc<Mutex<WakerRegistry>>,
}

impl<S: Stream> ForkedInner<S>
where
	S::Item: Clone,
{
	fn init(source: S) -> Forked<S> {
		let inner = Self {
			source: source.fuse(),
			buffer: Buffer::new(),
			wakers: Arc::new(Mutex::new(WakerRegistry::default())),
		};
		let arc = Arc::new(Mutex::new(Box::pin(inner)));

		ForkedInner::create_fork(arc, None)
	}

	fn create_fork(
		this: Arc<Mutex<Pin<Box<Self>>>>,
		with_offset_of: Option<&BufferSubscriber>,
	) -> Forked<S> {
		let subscriber = this
			.lock()
			.unwrap()
			.as_mut()
			.project()
			.buffer
			.create_subscriber(with_offset_of);

		Forked {
			subscriber: Some(subscriber),
			inner: this,
		}
	}

	fn lock_inner(this: &Arc<Mutex<Pin<Box<Self>>>>) -> MutexGuard<'_, Pin<Box<Self>>> {
		this.lock().unwrap()
	}
}

/// A cloneable handle reading its own copy of every item the source yields.
///
/// Clones start at this fork's current position; [`Forked::downgrade`]
/// produces a [`Weak`] handle whose upgrades start at the live edge.
pub struct Forked<S: Stream>
where
	S::Item: Clone,
{
	subscriber: Option<BufferSubscriber>,
	inner: Arc<Mutex<Pin<Box<ForkedInner<S>>>>>,
}

impl<S: Stream> Clone for Forked<S>
where
	S::Item: Clone,
{
	fn clone(&self) -> Self {
		ForkedInner::create_fork(
			self.inner.clone(),
			Some(self.subscriber.as_ref().expect("only unset during drop")),
		)
	}
}

impl<S: Stream> Stream for Forked<S>
where
	S::Item: Clone,
{
	type Item = S::Item;

	fn poll_next(
		self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> Poll<Option<Self::Item>> {
		let this = Pin::get_mut(self);
		let wakers = ForkedInner::lock_inner(&this.inner)
			.as_mut()
			.project()
			.wakers
			.clone();
		let source_waker = Waker::from(Arc::new(SourceWaker(Arc::clone(&wakers))));

		let result = {
			let mut inner = ForkedInner::lock_inner(&this.inner);
			let projected = inner.as_mut().project();
			let subscriber = this.subscriber.as_mut().expect("only unset during drop");

			if let Some(item) = projected.buffer.read(subscriber) {
				return Poll::Ready(Some(item));
			}

			// Park this fork with the registry, then poll the source under
			// the shared waker so *any* later reader that finds the source
			// ready wakes this fork too.
			wakers.lock().unwrap().park(cx.waker());
			let mut context = std::task::Context::from_waker(&source_waker);

			match Stream::poll_next(projected.source, &mut context) {
				Poll::Ready(Some(item)) => {
					projected.buffer.push(item);

					Poll::Ready(Some(
						projected
							.buffer
							.read(subscriber)
							.expect("the item was just pushed into the buffer"),
					))
				}
				Poll::Ready(None) => Poll::Ready(None),
				Poll::Pending => Poll::Pending,
			}
		};

		// Wakes happen outside the inner lock: a woken fork re-enters
		// `poll_next` on its own task, never through this held lock.
		if result.is_ready() {
			wakers.lock().unwrap().wake_all();
		}

		result
	}
}

impl<S: Stream> Drop for Forked<S>
where
	S::Item: Clone,
{
	fn drop(&mut self) {
		let subscriber = self.subscriber.take().expect("only unset during drop");

		ForkedInner::lock_inner(&self.inner)
			.as_mut()
			.project()
			.buffer
			.dispose_of_subscriber(subscriber);
	}
}

impl<S: Stream> Forked<S>
where
	S::Item: Clone,
{
	/// A weak handle whose [`Weak::upgrade`] starts at the live edge — the
	/// next frame to arrive — rather than this fork's read position.
	pub fn downgrade(&self) -> Weak<S> {
		Weak(Arc::downgrade(&self.inner))
	}
}

/// A weak reference to a forked stream; must be upgraded before it can be
/// polled.
pub struct Weak<S: Stream>(std::sync::Weak<Mutex<Pin<Box<ForkedInner<S>>>>>)
where
	S::Item: Clone;

impl<S: Stream> Weak<S>
where
	S::Item: Clone,
{
	/// Upgrade to a fork positioned at the live edge of the shared buffer.
	///
	/// Fails only when every fork — including the never-read root — is gone.
	pub fn upgrade(&self) -> Option<Forked<S>> {
		self.0
			.upgrade()
			.map(|inner| ForkedInner::create_fork(inner, None))
	}
}

// The buffer below is carried over from fork_stream 0.1.0 unchanged: a
// garbage-collected queue where each entry lives until every subscriber
// ahead of it has read it. The lint allows keep the vendored code verbatim.
#[allow(clippy::struct_field_names)]
struct Buffer<T: Clone> {
	buf: VecDeque<BufferEntry<T>>,
	buffer_start_offset: usize,
	num_subscribers: usize,
}

struct BufferSubscriber {
	offset: usize,
}

struct BufferEntry<T> {
	item: T,
	refs: usize,
}

impl<T: Clone> Buffer<T> {
	fn new() -> Self {
		Self {
			buf: VecDeque::new(),
			buffer_start_offset: 0,
			num_subscribers: 0,
		}
	}

	fn create_subscriber(&mut self, with_offset_of: Option<&BufferSubscriber>) -> BufferSubscriber {
		self.num_subscribers += 1;

		let offset = if let Some(with_offset_of) = with_offset_of {
			for entry in self
				.buf
				.iter_mut()
				.skip(with_offset_of.offset - self.buffer_start_offset)
			{
				entry.refs += 1;
			}
			with_offset_of.offset
		} else {
			self.buffer_start_offset + self.buf.len()
		};

		BufferSubscriber { offset }
	}

	fn push(&mut self, item: T) {
		self.buf.push_back(BufferEntry {
			item,
			refs: self.num_subscribers,
		});
	}

	fn read(&mut self, subscriber: &mut BufferSubscriber) -> Option<T> {
		if let Some(entry) = self
			.buf
			.get_mut(subscriber.offset - self.buffer_start_offset)
		{
			entry.refs -= 1;

			let item = if entry.refs == 0 {
				let item = self
					.buf
					.pop_front()
					.expect("there is at least one item in the buffer")
					.item;
				self.buffer_start_offset += 1;
				item
			} else {
				entry.item.clone()
			};

			subscriber.offset += 1;

			Some(item)
		} else {
			None
		}
	}

	#[allow(clippy::needless_pass_by_value)]
	fn dispose_of_subscriber(&mut self, subscriber: BufferSubscriber) {
		self.num_subscribers -= 1;

		let mut entries_to_remove = 0_usize;
		for entry in self
			.buf
			.iter_mut()
			.skip(subscriber.offset - self.buffer_start_offset)
		{
			entry.refs -= 1;
			if entry.refs == 0 {
				entries_to_remove += 1;
			}
		}

		for _ in 0..entries_to_remove {
			self.buf.pop_front();
		}

		self.buffer_start_offset += entries_to_remove;
	}
}

#[cfg(all(test, feature = "ssr", not(target_arch = "wasm32")))]
mod tests {

	use std::task::Poll;

	use assert2::check;
	use futures::StreamExt;
	use futures::executor::block_on;
	use futures::future::poll_fn;
	use futures::stream;

	use super::ForkStreamExt as _;

	/// The starvation this fork fixes: fork A parks, fork B polls the source
	/// (so B's waker replaces everyone's with the source) and is then
	/// dropped, and an item arrives. The published crate wakes only the
	/// source-registered — now dead — waker, so A sleeps forever; here A must
	/// still receive the item.
	#[tokio::test]
	#[allow(clippy::similar_names)]
	async fn parked_fork_survives_another_poller_dropping() {
		let (sender, receiver) = futures::channel::mpsc::unbounded::<u32>();
		let root = receiver.fork();
		let mut fork_a = root.clone();
		let mut fork_b = root.clone();
		drop(root);

		// A parks on an empty buffer.
		let first_parked =
			poll_fn(|cx| Poll::Ready(matches!(fork_a.poll_next_unpin(cx), Poll::Pending))).await;
		check!(first_parked);

		// B polls the source once — registering the source under the shared
		// waker — then disappears without ever waking anyone.
		let second_parked =
			poll_fn(|cx| Poll::Ready(matches!(fork_b.poll_next_unpin(cx), Poll::Pending))).await;
		check!(second_parked);
		drop(fork_b);

		// The item arrives; A must be woken through the shared waker and read
		// it without any further polling from anywhere else.
		sender.unbounded_send(7).unwrap();
		let received = tokio::time::timeout(std::time::Duration::from_secs(2), fork_a.next())
			.await
			.expect("the parked fork is woken")
			.expect("the stream has not ended");

		check!(received == 7);
	}

	/// Clones read the full history from their creation offset; weak upgrades
	/// start at the live edge.
	#[test]
	fn clones_replay_and_weak_upgrades_start_at_live_edge() {
		let root = stream::iter(0_u32..3).fork();
		let weak = root.downgrade();
		let mut replaying = root.clone();

		// Drain the root; the clone still sees everything.
		let drained: Vec<u32> = block_on(root.take(3).collect());
		check!(drained == vec![0, 1, 2]);

		let replayed: Vec<u32> = block_on(replaying.by_ref().take(2).collect());
		check!(replayed == vec![0, 1]);

		let mut live = weak.upgrade().expect("the root still exists");
		let at_edge = block_on(live.next());
		check!(at_edge.is_none());
	}

	/// Upgrading fails once every fork — including the never-read root — is
	/// gone; the provider relies on this never happening while it lives.
	#[test]
	fn weak_upgrade_fails_without_any_fork() {
		let root = stream::iter(0_u32..1).fork();
		let weak = root.downgrade();

		check!(weak.upgrade().is_some());

		drop(root);

		check!(weak.upgrade().is_none());
	}

	/// Items are dropped once every subscriber ahead of them has read them.
	#[test]
	fn buffer_releases_consumed_items() {
		let root = stream::iter(0_u32..3).fork();
		let mut slow = root.clone();
		let mut fast = root.clone();
		drop(root);

		check!(block_on(fast.next()) == Some(0));
		check!(block_on(fast.next()) == Some(1));
		check!(block_on(slow.next()) == Some(0));
		check!(block_on(fast.next()) == Some(2));
		check!(block_on(slow.next()) == Some(1));
		check!(block_on(slow.next()) == Some(2));
	}
}
