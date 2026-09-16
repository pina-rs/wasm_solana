use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use typed_builder::TypedBuilder;

use crate::ClientWebSocketError;

/// A JSON-RPC 2.0 request envelope shared by the HTTP and websocket
/// transports.
#[derive(Debug, Clone, Serialize, TypedBuilder)]
pub struct ClientRequest {
	/// JSON-RPC protocol version, always `"2.0"`.
	#[builder(default = "2.0")]
	pub jsonrpc: &'static str,
	/// Request id echoed back by the node. HTTP calls reuse a fixed id, while
	/// each websocket subscription allocates its own.
	#[builder(default)]
	pub id: u32,
	/// Solana RPC method name, such as `getBalance` or `accountSubscribe`.
	#[builder(setter(into))]
	pub method: String,
	/// Method arguments sent as the JSON-RPC `params` value.
	///
	/// Omitted from the payload when it would carry no information: either
	/// `null`, or an array whose elements are all `null`. An empty array is
	/// therefore also omitted.
	#[serde(skip_serializing_if = "is_null")]
	#[builder(default = Value::Null, setter(transform = |value: impl Serialize| serde_json::to_value(value).unwrap_or_default()))]
	pub params: Value,
}

impl ClientRequest {
	/// Serialize the request into a JSON value for the websocket transport.
	pub fn try_to_value(&self) -> Result<Value, ClientWebSocketError> {
		serde_json::to_value(self).map_err(|_| ClientWebSocketError::InvalidMessage)
	}
}

/// Identifier the node assigns to a websocket subscription.
pub type SubscriptionId = u64;

/// A websocket notification carrying a subscription response.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionResponse<T> {
	/// JSON-RPC protocol version, always `"2.0"`.
	pub jsonrpc: String,
	/// Notification method name, such as `accountNotification`.
	pub method: String,
	/// The notification payload together with the subscription it belongs to.
	pub params: SubscriptionParams<T>,
}

/// The body of a websocket notification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscriptionParams<T> {
	/// The notification payload, whose shape depends on the subscription.
	pub result: T,
	/// The subscription that produced this notification.
	pub subscription: SubscriptionId,
}

/// A JSON-RPC response envelope returned by the HTTP transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientResponse<T> {
	/// JSON-RPC protocol version, always `"2.0"`.
	pub jsonrpc: String,
	/// The method result.
	pub result: T,
	/// The id of the request this response answers.
	pub id: u32,
}

/// Response to a subscription request, carrying the new subscription id.
pub type SubscriptionResult = ClientResponse<SubscriptionId>;
/// Response to an unsubscribe request, reporting whether it took effect.
pub type UnsubscriptionResult = ClientResponse<bool>;

/// Number of polls transaction confirmation performs before giving up.
pub const MAX_RETRIES: usize = 25;
/// Delay between confirmation polls, approximating the Solana block time.
pub const SLEEP_MS: u64 = 400; // solana block time

fn is_null(v: &Value) -> bool {
	match v {
		Value::Null => true,
		Value::Array(array) => array.iter().all(Value::is_null),
		_ => false,
	}
}
