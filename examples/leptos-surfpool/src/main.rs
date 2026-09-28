//! Leptos browser demo for `wasm_client_solana`, tested end-to-end with
//! Playwright against a local [surfpool](https://surfpool.run) node.
//!
//! Build (from this directory, then `examples/` for the wasm-bindgen step —
//! the repository's `wasm-bindgen` shim resolves from there):
//!
//! ```sh
//! cargo build --release --target wasm32-unknown-unknown
//! # from examples/:
//! ../.bin/.shims/wasm-bindgen --target web --out-dir leptos-surfpool/dist \
//! 	leptos-surfpool/target/wasm32-unknown-unknown/release/leptos-surfpool-example.wasm
//! cp leptos-surfpool/index.html leptos-surfpool/dist/
//! ```
//!
//! or `npm run build:leptos` from `examples/`, or `trunk serve` here.

use std::str::FromStr;

use futures::StreamExt;
use leptos::prelude::*;
use solana_keypair::Keypair;
use solana_native_token::sol_str_to_lamports;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;
use wasm_client_solana::SolanaRpcClient;
use wasm_client_solana::spawn_local;

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
	leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
	// Constructing the client eagerly opens the pubsub websocket (derived
	// from RPC_URL as ws://127.0.0.1:8900), so surfpool must already be
	// running when the page loads.
	let rpc = SolanaRpcClient::new(RPC_URL);
	let wallet = demo_keypair();

	let (status, set_status) = signal("connecting".to_string());
	let (node_version, set_node_version) = signal(String::new());
	let (balance, set_balance) = signal(Option::<u64>::None);
	let (last_signature, set_last_signature) = signal(String::new());
	let (transfer_status, set_transfer_status) = signal(String::new());
	let (notifications, set_notifications) = signal(0u64);
	let (last_slot, set_last_slot) = signal(0u64);
	let (busy, set_busy) = signal(false);

	// Initial load: prove the HTTP transport works with a version + balance
	// read before flipping the status to `connected`.
	{
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		spawn_local(async move {
			match rpc.get_version().await {
				Ok(version) => {
					set_node_version.set(format!(
						"{} (feature-set {:?})",
						version.solana_core, version.feature_set
					));
				}
				Err(error) => set_status.set(format!("error: {error}")),
			}
			match rpc.get_balance(&pubkey).await {
				Ok(lamports) => {
					set_balance.set(Some(lamports));
					set_status.set("connected".to_string());
				}
				Err(error) => set_status.set(format!("error: {error}")),
			}
		});
	}

	// Live updates: subscribe to the demo account over the websocket and
	// mirror every notification (balance changes included) into the UI.
	{
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		spawn_local(async move {
			let mut subscription = match rpc.account_subscribe(pubkey).await {
				Ok(subscription) => subscription,
				Err(error) => {
					set_status.set(format!("subscription error: {error}"));
					return;
				}
			};

			while let Some(notification) = subscription.next().await {
				set_notifications.update(|count| *count += 1);
				set_last_slot.set(notification.params.result.context.slot);
				if let Some(account) = notification.params.result.value {
					set_balance.set(Some(account.lamports));
				}
			}
		});
	}

	// Airdrop 1 SOL to the demo wallet, confirm it, then refresh the balance.
	let on_airdrop = {
		let rpc = rpc.clone();
		let pubkey = wallet.pubkey();
		move |_| {
			set_busy.set(true);
			let rpc = rpc.clone();
			spawn_local(async move {
				let result = async {
					let signature = rpc.request_airdrop(&pubkey, AIRDROP_LAMPORTS).await?;
					let _confirmed = rpc.confirm_transaction(&signature).await;
					let lamports = rpc.get_balance(&pubkey).await?;
					Ok::<_, wasm_client_solana::ClientError>((signature, lamports))
				}
				.await;

				match result {
					Ok((signature, lamports)) => {
						set_last_signature.set(signature.to_string());
						set_balance.set(Some(lamports));
					}
					Err(error) => set_status.set(format!("error: {error}")),
				}
				set_busy.set(false);
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
			set_busy.set(true);
			let rpc = rpc.clone();
			// `Keypair` is not `Clone`; rebuilding it per click from the
			// constant bytes keeps this closure `FnMut`.
			let payer = demo_keypair();
			spawn_local(async move {
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

				set_transfer_status.set(match result {
					Ok((signature, true)) => format!("confirmed: {signature}"),
					Ok((signature, false)) => format!("not confirmed: {signature}"),
					Err(error) => format!("failed: {error}"),
				});
				set_busy.set(false);
			});
		}
	};

	view! {
		<main>
			<h1>"wasm_client_solana × Leptos × surfpool"</h1>
			<p>
				"status: " <span id="status">{move || status.get()}</span>
			</p>
			<p>
				"node version: " <span id="rpc-version">{move || node_version.get()}</span>
			</p>
			<p>
				"balance: "
				<span id="balance">
					{move || {
						balance
							.get()
							.map(|lamports| format!("{lamports} lamports"))
							.unwrap_or_else(|| "...".to_string())
					}}
				</span>
			</p>
			<p>
				"last signature: " <span id="last-signature">{move || last_signature.get()}</span>
			</p>
			<p>
				"transfer: " <span id="transfer-status">{move || transfer_status.get()}</span>
			</p>
			<p>
				"notifications: " <span id="notifications">{move || notifications.get()}</span>
				" (last slot " <span id="last-slot">{move || last_slot.get()}</span> ")"
			</p>
			<button id="airdrop" prop:disabled=move || busy.get() on:click=on_airdrop>
				"Airdrop 1 SOL"
			</button>
			<button id="transfer" prop:disabled=move || busy.get() on:click=on_transfer>
				"Send 0.1 SOL"
			</button>
		</main>
	}
}
