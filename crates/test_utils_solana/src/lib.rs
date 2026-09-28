//! Testing utilities for Solana programs and clients.
//!
//! Provides a `TestValidatorRunner` (behind the `test_validator` feature) that
//! boots a local validator, a [`ProgramTest`]/[`ProgramTestContext`] wrapper
//! with helpers for funding accounts and adding programs, and a
//! [`TestRpcProvider`] that serves a [`ProgramTestContext`] over the
//! [`RpcProvider`](wasm_client_solana::RpcProvider) trait.

pub use solana_banks_client::BanksClientExt;
pub use solana_banks_interface::BanksTransactionResultWithSimulation;
pub use solana_program_binaries as programs;
pub use solana_program_runtime;
pub use solana_program_test::BanksClient;
pub use solana_program_test::BanksClientError;
pub use solana_program_test::ProgramTest;
pub use solana_program_test::ProgramTestBanksClientExt;
pub use solana_program_test::ProgramTestContext;
pub use solana_program_test::ProgramTestError;
pub use solana_program_test::processor;
pub use test_rpc_provider::*;
#[cfg(feature = "test_validator")]
pub use test_validator_runner::*;
pub use utils::*;

mod macros;
mod test_rpc_provider;
#[cfg(feature = "test_validator")]
mod test_validator_runner;
mod utils;

/// Commonly used traits and types for writing tests against a banks client
/// or [`ProgramTest`].
pub mod prelude {
	pub use wallet_standard::prelude::*;
	pub use wasm_client_solana::prelude::*;

	pub use super::BanksClientAsyncExtension;
	pub use super::ProgramTestBanksClientExt;
	pub use super::ProgramTestContextExtension;
	pub use super::ProgramTestExtension;
}

#[doc(hidden)]
pub mod __private {
	pub use assert2::check;
}
