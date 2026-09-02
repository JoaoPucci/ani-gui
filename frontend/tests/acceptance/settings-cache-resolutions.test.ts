// Acceptance: the Settings page carries the resolution-caching
// opt-in — rendered off by default, and flipping it persists
// cache_resolutions through the settings endpoint.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';

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

import SettingsPage from '../../src/routes/settings/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const APP_INFO = {
	version: '0.12.1',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337',
	removed_legacy_paths: []
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
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

const toggle = () =>
	target.querySelector(
		`input[aria-label="${m.settings_cache_resolutions_aria_label()}"]`
	) as HTMLInputElement | null;

describe('settings — the resolution-caching opt-in', () => {
	it('renders off by default and persists the opt-in', async () => {
		let putBody: Record<string, unknown> | null = null;
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.put(`${API_BASE}/api/settings`, async ({ request }) => {
				putBody = (await request.json()) as Record<string, unknown>;
				return new HttpResponse(null, { status: 204 });
			}),
			http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(APP_INFO)),
			http.get(`${API_BASE}/api/account/status`, () =>
				HttpResponse.json({ anilist: null, mal: null })
			)
		);

		app = mount(SettingsPage, { target });
		await until(() => toggle() !== null, 'the caching toggle to render');
		expect(toggle()!.checked).toBe(false);

		toggle()!.click();
		await until(() => putBody !== null, 'the settings write');
		expect(putBody!.cache_resolutions).toBe(true);
	});
});
