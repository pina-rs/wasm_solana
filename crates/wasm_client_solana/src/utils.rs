use std::future::Future;

/// Spawn a future on the runtime available to the target.
///
/// Uses `wasm_bindgen_futures` under the `js` feature and a tokio local set
/// under `ssr`. Both schedule the future and return immediately, so this does
/// not wait for it to finish. With neither feature the future is driven to
/// completion on the current thread with the futures executor, which does block
/// until it finishes.
///
/// # Panics under `ssr` without a `LocalSet`
///
/// The `ssr` branch calls [`tokio::task::spawn_local`], which panics outside a
/// [`tokio::task::LocalSet`] context. Run the reactor inside
/// `LocalSet::run_until` (or `#[tokio::main(flavor = "current_thread")]` with
/// a LocalSet), or require `Send` futures and use `tokio::task::spawn`
/// directly — as the pubsub provider's own reader does.
pub fn spawn_local<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	cfg_if::cfg_if! {
		if #[cfg(feature = "js")] {
			wasm_bindgen_futures::spawn_local(fut);
		} else if #[cfg(feature = "ssr")] {
			tokio::task::spawn_local(fut);
		}  else {

			futures::executor::block_on(fut);
		}
	}
}

pub(crate) fn get_ws_url(url: impl Into<String>) -> String {
	let mut url: String = url.into();

	if url.starts_with("http") {
		let first_index = url.find(':').expect("Invalid URL");

		url.replace_range(
			..first_index,
			if url.starts_with("https") {
				"wss"
			} else {
				"ws"
			},
		);

		// Increase the port number by 1 if the port is specified. The
		// checked add refuses to wrap: an endpoint on u16::MAX has no
		// pubsub port to derive, and silently wrapping to a low port would
		// point pubsub at an unrelated service.
		let last_index = url.rfind(':').unwrap();

		if last_index != first_index
			&& let Some(Ok(port)) = url.get(last_index + 1..).map(str::parse::<u16>)
			&& let Some(pubsub_port) = port.checked_add(1)
		{
			url.replace_range(last_index + 1.., &pubsub_port.to_string());
		}
	}

	url
}
