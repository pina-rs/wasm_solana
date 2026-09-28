//! HTTP and websocket transports, split between wasm and native backends.
//!
//! Both implement [`RpcProvider`](crate::RpcProvider): HTTP carries the
//! request/response JSON-RPC calls and websocket carries the pubsub
//! subscriptions. Each has a `gloo-net` build for wasm targets and a `reqwest`
//! build for the `ssr` feature.

pub use http_provider::*;
pub use websocket_provider::*;

mod http_provider;
mod websocket_provider;
