/**
 * How a spec answers a request it stubs: over real HTTP, through the
 * shared stub server (lib/e2e-stub-server.cjs).
 *
 * A spec intercepts the renderer's request with a route, as before, and
 * hands it to `answer` with what the backend should say. The answer is
 * held at the stub server, and the request is sent on to it. It then
 * travels through Chromium's network stack as a request to the backend
 * does, and is answered behind the backend's gate and CORS rules: an
 * API request without the launch's secret gets the backend's empty 403,
 * not the stub. An answer made in place inside Playwright loses its
 * status and headers whenever the main process watches the window's
 * requests (tests/arch/e2e_stub_server.sh), so no spec answers a request
 * in place. (Playwright still answers a CORS preflight itself, for any
 * origin; the stub server's CORS rules apply to the answer.)
 */
import type { Page, Route } from '@playwright/test';
import { startStubServer } from '../lib/e2e-stub-server.cjs';

/** What a stub says: Playwright's answer shape. */
export type StubAnswer = {
	status?: number;
	headers?: Record<string, string>;
	contentType?: string;
	body?: string | Buffer;
	json?: unknown;
};

type StubServer = Awaited<ReturnType<typeof startStubServer>>;

/** One server for the worker's whole run. */
let server: Promise<StubServer> | null = null;

/** The launch's secret, per window, once read. */
const secrets = new WeakMap<Page, string>();

async function readSecret(page: Page): Promise<string | null> {
	const secret = await page
		.mainFrame()
		.evaluate(
			() => (window as { aniGui?: { internalSecret?: string } }).aniGui?.internalSecret ?? null
		);
	return typeof secret === 'string' && secret.length > 0 ? secret : null;
}

/** The secret of the launch whose window made `route`'s request, as the
 *  preload handed it to the app's page; null when that cannot be read
 *  (no window, a page that is not the app's), and the gate then refuses
 *  every API answer, as the backend would without the secret. */
async function launchSecret(route: Route): Promise<string | null> {
	let page: Page;
	try {
		page = route.request().frame().page();
	} catch {
		return null; // a request no frame made
	}
	const known = secrets.get(page);
	if (known) return known;
	let secret: string | null = null;
	try {
		secret = await readSecret(page);
	} catch {
		// The page was between documents: read again once the next is in.
		await page.waitForLoadState('domcontentloaded').catch(() => {});
		secret = await readSecret(page).catch(() => null);
	}
	if (secret) secrets.set(page, secret);
	return secret;
}

/** Answers `route`'s request with `response`, over HTTP, as the backend
 *  would: see the module comment. */
export async function answer(route: Route, response: StubAnswer = {}): Promise<void> {
	const stub = await (server ??= startStubServer());
	const url = new URL(route.request().url());
	const target = stub.hold(response, {
		path: url.pathname,
		query: url.search.slice(1),
		secret: await launchSecret(route)
	});
	await route.continue({ url: target });
}
