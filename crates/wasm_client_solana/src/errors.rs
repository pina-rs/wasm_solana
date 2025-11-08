use std::fmt;

use serde::Deserialize;
use serde::Serialize;
use solana_message::CompileError;
use solana_signer::SignerError;
use wallet_standard::IntoWalletError;
use wallet_standard::WalletError;

use crate::nonce_utils::NonceError;

pub const DEFAULT_ERROR_CODE: u16 = 500u16;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RpcErrorDetails {
	pub(crate) code: i32,
	pub(crate) message: String,
}

impl Default for RpcErrorDetails {
	fn default() -> Self {
		let message = "Internal Server Error".into();
		let code = DEFAULT_ERROR_CODE.into();

		Self { code, message }
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcError {
	pub(crate) id: u32,
	pub(crate) jsonrpc: String,
	pub(crate) error: RpcErrorDetails,
}

impl std::error::Error for RpcError {}

impl Default for RpcError {
	fn default() -> Self {
		Self {
			id: 0,
			jsonrpc: String::from("2.0"),
			error: RpcErrorDetails::default(),
		}
	}
}

impl RpcError {
	pub fn new(message: impl Into<String>) -> Self {
		let message = message.into();
		let code = 303;
		let error = RpcErrorDetails { code, message };

		RpcError {
			error,
			..Default::default()
		}
	}
}

impl fmt::Display for RpcError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(format!("Client error: {}", self.error.message).as_str())
	}
}

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Clone, Debug, Serialize, Deserialize, thiserror::Error)]
pub enum ClientError {
	/// An rpc client error.
	#[error("{0}")]
	Rpc(#[from] RpcError),
	#[error("Websocket Error: {0}")]
	WebSocket(#[from] ClientWebSocketError),
	/// The wallet error.
	#[error("{0}")]
	Wallet(#[from] WalletError),
	/// The nonce error.
	#[error("{0}")]
	Nonce(#[from] NonceError),
	/// The string of any unsupported errors.
	#[error("Other: {0}")]
	Other(String),
}

impl IntoWalletError for ClientError {}
impl IntoWalletError for ClientWebSocketError {}
impl IntoWalletError for RpcError {}

/// Error returned by WebSocket
#[derive(Clone, Copy, Debug, Serialize, Deserialize, thiserror::Error)]
#[non_exhaustive]
pub enum ClientWebSocketError {
	/// The `error` event
	#[error("connection error")]
	ConnectionError,
	/// The `close` event
	#[error("the connection closed")]
	ConnectionClose,
	/// Message failed to send.
	#[error("there was an error sending the message")]
	MessageSendError,
	/// The message could not be deserialized
	#[error("the message could not be deserialized")]
	InvalidMessage,
	/// The message could not be subscribed
	#[error("could not subscribe to message")]
	Subscription,
	/// The message could not be subscribed
	#[error("could not unsubscribe from messages")]
	Unsubscription,
}

impl From<gloo_net::websocket::WebSocketError> for ClientWebSocketError {
	fn from(value: gloo_net::websocket::WebSocketError) -> Self {
		ClientWebSocketError::from(&value)
	}
}

impl From<SignerError> for ClientError {
	fn from(value: SignerError) -> Self {
		Self::Other(format!("Signer: {value}"))
	}
}

impl From<CompileError> for ClientError {
	fn from(value: CompileError) -> Self {
		Self::Other(format!("Compile: {value}"))
	}
}

impl From<&gloo_net::websocket::WebSocketError> for ClientWebSocketError {
	fn from(value: &gloo_net::websocket::WebSocketError) -> Self {
		match value {
			gloo_net::websocket::WebSocketError::ConnectionError => Self::ConnectionError,
			gloo_net::websocket::WebSocketError::ConnectionClose(_) => Self::ConnectionClose,
			gloo_net::websocket::WebSocketError::MessageSendError(_) => Self::MessageSendError,
			_ => Self::InvalidMessage,
		}
	}
}
