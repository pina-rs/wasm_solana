use async_trait::async_trait;
use serde_json::Value;
#[cfg(feature = "ssr")]
pub use ssr_http_provider::HttpProvider;
#[cfg(not(feature = "ssr"))]
pub use wasm_http_provider::HttpProvider;

use crate::ClientRequest;
use crate::ClientResult;

#[async_trait]
pub trait RpcProvider {
	/// Send the request.
	async fn send(&self, method: &'static str, request: Value) -> ClientResult<Value>;
	/// Get the URL represented by this sender.
	fn url(&self) -> String;
}

#[cfg(feature = "ssr")]
mod ssr_http_provider {
	use reqwest::Client;
	use reqwest::header::CONTENT_TYPE;
	use reqwest::header::HeaderMap;

	use super::*;
	use crate::ClientError;
	use crate::DEFAULT_ERROR_CODE;
	use crate::RpcError;
	use crate::RpcErrorDetails;

	#[derive(Debug, Clone)]
	pub struct HttpProvider {
		client: Client,
		headers: HeaderMap,
		url: String,
	}

	#[async_trait]
	impl RpcProvider for HttpProvider {
		fn url(&self) -> String {
			self.url.clone()
		}

		async fn send(&self, method: &'static str, request: Value) -> ClientResult<Value> {
			let client_request = ClientRequest::builder()
				.method(method)
				.id(1)
				.params(request)
				.build();
			#[cfg(not(target_arch = "wasm32"))]
			let result: Value = self
				.client
				.post(&self.url)
				.headers(self.headers.clone())
				.json(&client_request)
				.send()
				.await?
				.json()
				.await?;

			#[cfg(target_arch = "wasm32")]
			let result: Value = {
				let request = self
					.client
					.post(&self.url)
					.headers(self.headers.clone())
					.json(&client_request)
					.send();
				let wrapped_request = send_wrapper::SendWrapper::new(request);
				let response = wrapped_request.await?.json();
				let wrapped_response = send_wrapper::SendWrapper::new(response);
				let result = wrapped_response.await?;
				result
			};

			Ok(result)
		}
	}

	impl HttpProvider {
		pub fn new(url: impl Into<String>) -> Self {
			let client = Client::new();
			let url = url.into();
			let mut headers = HeaderMap::new();
			headers.append(CONTENT_TYPE, "application/json".parse().unwrap());

			Self {
				client,
				headers,
				url,
			}
		}
	}

	impl From<reqwest::Error> for RpcError {
		fn from(error: reqwest::Error) -> Self {
			let message = error.to_string();
			let code = i32::from(error.status().map_or(DEFAULT_ERROR_CODE, |s| s.as_u16()));
			let error = RpcErrorDetails { code, message };

			RpcError {
				error,
				..Default::default()
			}
		}
	}

	impl From<reqwest::Error> for ClientError {
		fn from(value: reqwest::Error) -> Self {
			ClientError::Rpc(value.into())
		}
	}
}

#[cfg(not(feature = "ssr"))]
mod wasm_http_provider {

	use std::pin::Pin;
	use std::task::Context;
	use std::task::Poll;

	use futures::Future;
	use pin_project::pin_project;
	use pin_project::pinned_drop;
	use send_wrapper::SendWrapper;
	use wasm_bindgen::prelude::*;
	use web_sys::AbortController;

	use super::*;
	use crate::ClientError;

	#[pin_project(PinnedDrop)]
	struct AbortableRequest<F: Future<Output = Result<gloo_net::http::Response, gloo_net::Error>>> {
		#[pin]
		fut: F,
		controller: AbortController,
		pending: bool,
	}

	impl<F: Future<Output = Result<gloo_net::http::Response, gloo_net::Error>>> AbortableRequest<F> {
		fn new(fut: F, controller: AbortController) -> Self {
			Self {
				fut,
				controller,
				pending: true,
			}
		}
	}

	impl<F: Future<Output = Result<gloo_net::http::Response, gloo_net::Error>>> Future
		for AbortableRequest<F>
	{
		type Output = Result<gloo_net::http::Response, gloo_net::Error>;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			let mut this = self.project();

			match this.fut.as_mut().poll(cx) {
				Poll::Ready(value) => {
					*this.pending = false;
					Poll::Ready(value)
				}

				Poll::Pending => Poll::Pending,
			}
		}
	}

	#[pinned_drop]
	impl<F: Future<Output = Result<gloo_net::http::Response, gloo_net::Error>>> PinnedDrop
		for AbortableRequest<F>
	{
		fn drop(self: Pin<&mut Self>) {
			if self.pending {
				// only abort the fetch if it is still pending.
				self.controller.abort();
			}
		}
	}

	#[derive(Debug, Clone)]
	pub struct HttpProvider(String);

	#[async_trait]
	impl RpcProvider for HttpProvider {
		fn url(&self) -> String {
			self.0.clone()
		}

		async fn send(&self, method: &'static str, request: Value) -> ClientResult<Value> {
			let client_request = ClientRequest::builder()
				.method(method)
				.id(0)
				.params(request)
				.build();

			let future = async move {
				let controller = AbortController::new().unwrap_throw();
				let signal = controller.signal();
				let request = gloo_net::http::Request::post(&self.0)
					.abort_signal(Some(&signal))
					.json(&client_request)?;
				let response = AbortableRequest::new(request.send(), controller).await?;
				let value = response.json().await?;

				Ok::<Value, ClientError>(value)
			};

			let value = SendWrapper::new(future).await?;

			Ok(value)
		}
	}

	impl HttpProvider {
		pub fn new(url: impl Into<String>) -> Self {
			Self(url.into())
		}
	}

	impl From<serde_wasm_bindgen::Error> for ClientError {
		fn from(value: serde_wasm_bindgen::Error) -> Self {
			Self::Other(value.to_string())
		}
	}
	impl From<gloo_net::Error> for ClientError {
		fn from(value: gloo_net::Error) -> Self {
			Self::Other(value.to_string())
		}
	}

	impl From<JsValue> for ClientError {
		fn from(error: JsValue) -> Self {
			Self::Other(
				error
					.as_string()
					.unwrap_or("An error occurred in the JavaScript.".to_string()),
			)
		}
	}
}

pub const DEVNET: &str = "https://api.devnet.solana.com";
pub const TESTNET: &str = "https://api.testnet.solana.com";
pub const MAINNET: &str = "https://api.mainnet-beta.solana.com";
pub const LOCALNET: &str = "http://127.0.0.1:8899";
pub const DEBUG: &str = "http://34.90.18.145:8899";
