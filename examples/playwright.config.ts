import { defineConfig } from "@playwright/test";

/**
 * End-to-end tests for the browser examples, run against a local surfpool
 * node (https://surfpool.run). Surfpool already speaks the JSON-RPC and
 * pubsub dialect of a test validator: HTTP on 8899, websocket on 8900,
 * `requestAirdrop` included, which is exactly what `wasm_client_solana`
 * derives from `http://127.0.0.1:8899`.
 *
 * The static apps are served with python's http.server so the whole suite
 * needs no bundler: `cargo build` + `wasm-bindgen --target web` output is
 * served as-is.
 */
export default defineConfig({
	testDir: "./e2e",
	timeout: 60_000,
	expect: {
		timeout: 15_000,
	},
	// Both projects share one surfpool wallet, so keep the runs serial.
	workers: 1,
	retries: process.env.CI ? 1 : 0,
	reporter: [["list"]],
	outputDir: "./test-results",
	webServer: [
		{
			command: "surfpool start --offline --yes --no-deploy",
			// surfpool answers GET / with 405 (it only accepts POST), so use a
			// TCP port probe instead of an HTTP url health check.
			port: 8899,
			reuseExistingServer: true,
			timeout: 60_000,
			stdout: "ignore",
			stderr: "ignore",
		},
		{
			command: "python3 -m http.server 4173 --directory leptos-surfpool/dist",
			url: "http://127.0.0.1:4173",
			reuseExistingServer: true,
		},
		{
			command: "python3 -m http.server 4174 --directory dioxus-surfpool/dist",
			url: "http://127.0.0.1:4174",
			reuseExistingServer: true,
		},
	],
	projects: [
		{
			name: "leptos",
			use: { baseURL: "http://127.0.0.1:4173" },
		},
		{
			name: "dioxus",
			use: { baseURL: "http://127.0.0.1:4174" },
		},
	],
});
