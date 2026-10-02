//! Dioxus browser demo for `wasm_client_solana`, tested end-to-end with
//! Playwright against a local [surfpool](https://surfpool.run) node.
//!
//! Build (from this directory, then `examples/` for the wasm-bindgen step —
//! the repository's `wasm-bindgen` shim resolves from there):
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown
//! # from examples/:
//! ../.bin/.shims/wasm-bindgen --target web --out-dir dioxus-surfpool/dist \
//! 	dioxus-surfpool/target/wasm32-unknown-unknown/release/dioxus-surfpool-example.wasm
//! cp dioxus-surfpool/index.html dioxus-surfpool/dist/
//! ```

use std::str::FromStr;

use dioxus::prelude::*;
use futures::StreamExt;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use wasm_client_solana::SolanaRpcClient;

/// Surfpool's default JSON-RPC port. The websocket URL is derived from it
/// (`http` -> `ws`, port + 1), which matches surfpool's default `--ws-port
/// 8900`.
const RPC_URL: &str = "http://127.0.0.1:8899";

/// Demo keypair, the same bytes as `test_utils_keypairs::SECRET_KEY_WALLET`
/// (`B6FAryxc6pfLuK4to1BuvhqVJQm2V1cthguAmSPvqPug`). A throwaway key for
/// local demos only — never ship a hardcoded secret in a real app.
const WALLET_SECRET_KEY: &[u8] = &[
	122, 234, 133, 232, 80, 195, 2, 115, 237, 183, 24, 30, 85, 198, 199, 101, 125, 18, 35, 185,
	237, 150, 219, 78, 22, 118, 140, 56, 55, 118, 119, 180, 149, 236, 203, 160, 52, 95, 187, 57,
	214, 47, 184, 28, 44, 195, 225, 83, 138, 219, 119, 32, 100, 18, 255, 236, 227, 160, 9, 12, 182,
	174, 75, 215,
];

/// `tBuug63EhqE836n1dirg3sZ5KwZuG6P6i5nDHYrryyX` (`SECRET_KEY_TREASURY`).
const RECIPIENT_PUBKEY: &str = "tBuug63EhqE836n1dirg3sZ5KwZuG6P6i5nDHYrryyX";

const AIRDROP_LAMPORTS: u64 = 1_000_000_000;

fn demo_keypair() -> Keypair {
	Keypair::try_from(WALLET_SECRET_KEY).expect("hardcoded demo keypair is valid")
}

fn main() {
	dioxus::launch(App);
}

#[component]
fn App() -> Element {
	// Constructing the client eagerly opens the pubsub websocket (derived
	// from RPC_URL as ws://127.0.0.1:8900), so surfpool must already be
	// running when the page loads.
	let rpc = use_hook(|| SolanaRpcClient::new(RPC_URL));
	// Rebuilt from the constant bytes on each render; avoids the `Clone`
	// bound `use_hook` places on its state (`Keypair` is not `Clone`).
	let wallet = demo_keypair();

	let mut status = use_signal(|| "connecting".to_string());
	let mut node_version = use_signal(String::new);
	let mut balance = use_signal(|| None::<u64>);
	let mut last_signature = use_signal(String::new);
	let mut transfer_status = use_signal(String::new);
	let mut notifications = use_signal(|| 0u64);
	let mut last_slot = use_signal(|| 0u64);
	let mut busy = use_signal(|| false);

	// One-shot startup tasks. The component body re-runs on every signal
	// change in Dioxus, so these live inside `use_hook` (which runs once per
	// component instance); without that every notification would re-run the
	// body and open a *new* account subscription per render.
	// Initial load: prove the HTTP transport works with a version + balance
	// read before flipping the status to `connected`.
	use_hook(|| {
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		spawn(async move {
			match rpc.get_version().await {
				Ok(version) => {
					node_version.set(format!(
						"{} (feature-set {:?})",
						version.solana_core, version.feature_set
					));
				}
				Err(error) => status.set(format!("error: {error}")),
			}
			match rpc.get_balance(&pubkey).await {
				Ok(lamports) => {
					balance.set(Some(lamports));
					status.set("connected".to_string());
				}
				Err(error) => status.set(format!("error: {error}")),
			}
		});
	});

	// Live updates: subscribe to the demo account over the websocket and
	// mirror every notification (balance changes included) into the UI.
	use_hook(|| {
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		spawn(async move {
			let subscription = match rpc.account_subscribe(pubkey).await {
				Ok(subscription) => subscription,
				Err(error) => {
					status.set(format!("subscription error: {error}"));
					return;
				}
			};

			let mut subscription = subscription;

			while let Some(notification) = subscription.next().await {
				notifications += 1;
				last_slot.set(notification.params.result.context.slot);
				if let Some(account) = notification.params.result.value {
					balance.set(Some(account.lamports));
				}
			}
		});
	});

	// Airdrop 1 SOL to the demo wallet, confirm it, then refresh the balance.
	let on_airdrop = {
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		move |_| {
			if busy() {
				return;
			}

			busy.set(true);
			let rpc = rpc.clone();
			spawn(async move {
				let result = async {
					let signature = rpc.request_airdrop(&pubkey, AIRDROP_LAMPORTS).await?;
					let _confirmed = rpc.confirm_transaction(&signature).await;
					let lamports = rpc.get_balance(&pubkey).await?;
					Ok::<_, wasm_client_solana::ClientError>((signature, lamports))
				}
				.await;

				match result {
					Ok((signature, lamports)) => {
						last_signature.set(signature.to_string());
						balance.set(Some(lamports));
					}
					Err(error) => status.set(format!("error: {error}")),
				}

				busy.set(false);
			});
		}
	};

	// Sign and send a 0.1 SOL transfer with the demo keypair, then poll for
	// confirmation.
	let on_transfer = {
		let rpc = rpc.clone();
		let recipient =
			Pubkey::from_str(RECIPIENT_PUBKEY).expect("hardcoded recipient pubkey is valid");
		move |_| {
			if busy() {
				return;
			}

			busy.set(true);
			let rpc = rpc.clone();
			// `Keypair` is not `Clone`; rebuilding it per click from the
			// constant bytes keeps this closure `FnMut`.
			let payer = demo_keypair();
			spawn(async move {
				let result = async {
					let blockhash = rpc.get_latest_blockhash().await?;
					let lamports = sol_str_to_lamports("0.1").expect("static amount parses");
					let instruction = transfer(&payer.pubkey(), &recipient, lamports);
					let transaction = Transaction::new_signed_with_payer(
						&[instruction],
						Some(&payer.pubkey()),
						&[&payer],
						blockhash,
					);
					let signature = rpc.send_transaction(&transaction.into()).await?;
					let confirmed = rpc.confirm_transaction(&signature).await?;
					Ok::<_, wasm_client_solana::ClientError>((signature, confirmed))
				}
				.await;

				transfer_status.set(match result {
					Ok((signature, true)) => format!("confirmed: {signature}"),
					Ok((signature, false)) => format!("not confirmed: {signature}"),
					Err(error) => format!("failed: {error}"),
				});

				busy.set(false);
			});
		}
	};

	rsx! {
		main {
			h1 { "wasm_client_solana × Dioxus × surfpool" }
			p {
				"status: "
				span { id: "status", "{status}" }
			}
			p {
				"node version: "
				span { id: "rpc-version", "{node_version}" }
			}
			p {
				"balance: "
				span {
					id: "balance",
					{balance().map(|lamports| format!("{lamports} lamports")).unwrap_or_else(|| "...".to_string())}
				}
			}
			p {
				"last signature: "
				span { id: "last-signature", "{last_signature}" }
			}
			p {
				"transfer: "
				span { id: "transfer-status", "{transfer_status}" }
			}
			p {
				"notifications: "
				span { id: "notifications", "{notifications}" }
				" (last slot "
				span { id: "last-slot", "{last_slot}" }
				")"
			}
			button {
				id: "airdrop",
				disabled: busy,
				onclick: on_airdrop,
				"Airdrop 1 SOL"
			}
			button {
				id: "transfer",
				disabled: busy,
				onclick: on_transfer,
				"Send 0.1 SOL"
			}
		}
	}
}
