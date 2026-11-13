// Acceptance: Diagnostics offers no page that plays a pasted URL.
//
// The page it used to link to handed a public URL straight to the
// player, the one place the renderer loaded a stream from anywhere but
// the backend's stream proxy. The renderer's content security policy
// admits media and connections from the backend's loopback lanes only,
// so that page could not work under it — and every stream is meant to
// pass through the proxy anyway. It is gone, and so is the way to it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import DiagnosticsPage from '../../src/routes/diagnostics/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const VERSION = '0.17.0';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: VERSION,
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: []
			})
		),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([]))
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

describe('diagnostics — no pasted-URL player', () => {
	it('links nowhere that plays a URL the user pastes', async () => {
		app = mount(DiagnosticsPage, { target });
		await until(() => (target.textContent ?? '').includes(VERSION), 'the page to load');

		// The player for a resolved episode lives under /play/<id>; the
		// pasted-URL page was /play itself.
		const hrefs = [...target.querySelectorAll('a[href]')].map((a) =>
			new URL(a.getAttribute('href') ?? '', 'http://app.invalid/diagnostics').pathname.replace(
				/\/+$/,
				''
			)
		);
		expect(hrefs, 'Diagnostics still links to the pasted-URL player').not.toContain('/play');
	});
});
