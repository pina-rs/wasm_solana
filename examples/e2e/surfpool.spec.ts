import { expect, type Page, test } from "@playwright/test";

/**
 * Shared spec, run once per framework project (leptos, dioxus). Both apps
 * expose the same DOM contract:
 *
 * - `#status` flips to `connected` once the HTTP transport answers
 *   `getVersion` + `getBalance` (and constructing the client also opened the
 *   pubsub websocket eagerly).
 * - `#balance` renders `<lamports> lamports`.
 * - `#airdrop` / `#transfer` buttons drive the full RPC flows.
 * - `#notifications` / `#last-slot` are fed by an `accountSubscribe`
 *   websocket subscription on the demo wallet.
 */
async function lamports(page: Page): Promise<number> {
	const text = await page.locator("#balance").innerText();
	const match = text.match(/(\d+) lamports/);

	if (!match) {
		throw new Error(`no lamports value in #balance: "${text}"`);
	}

	return Number(match[1]);
}

async function notificationCount(page: Page): Promise<number> {
	const text = await page.locator("#notifications").innerText();

	return Number(text);
}

test.beforeEach(async ({ page }) => {
	await page.goto("/");
	await expect(page.locator("#status")).toHaveText("connected", {
		timeout: 20_000,
	});
});

test("connects to surfpool over http", async ({ page }) => {
	await expect(page.locator("#rpc-version")).toHaveText(/\d+\.\d+\.\d+/);
	await expect(page.locator("#balance")).toHaveText(/\d+ lamports/);
});

test("airdrop credits the wallet", async ({ page }) => {
	const before = await lamports(page);

	await page.locator("#airdrop").click();

	await expect(page.locator("#last-signature")).toHaveText(
		/[1-9A-HJ-NP-Za-km-z]{80,}/,
		{
			timeout: 30_000,
		},
	);
	await expect
		.poll(() => lamports(page), { timeout: 30_000 })
		.toBe(before + 1_000_000_000);
});

test("account subscription receives live notifications", async ({ page }) => {
	const before = await notificationCount(page);

	await page.locator("#airdrop").click();

	// The airdrop mutates the subscribed account, so surfpool pushes an
	// accountNotification over the websocket.
	await expect
		.poll(() => notificationCount(page), { timeout: 30_000 })
		.toBeGreaterThan(before);
	await expect(page.locator("#last-slot")).toHaveText(/[1-9]\d*/);
});

test(
	"signs, sends and confirms a sol transfer",
	{ timeout: 90_000 },
	async ({ page }) => {
		// Fund first so the transfer never fails on an empty wallet.
		await page.locator("#airdrop").click();
		await expect
			.poll(() => lamports(page), { timeout: 30_000 })
			.toBeGreaterThan(0);

		await page.locator("#transfer").click();

		await expect(page.locator("#transfer-status")).toHaveText(
			/^confirmed: [1-9A-HJ-NP-Za-km-z]{80,}$/,
			{
				timeout: 60_000,
			},
		);
	},
);
