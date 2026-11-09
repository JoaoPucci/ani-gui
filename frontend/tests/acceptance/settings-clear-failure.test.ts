// Acceptance: a failed clear on Settings says what failed.
//
// Clear history and Clear image cache report a failure in the page's
// error block, which was headed "Couldn't save." for every failure. A
// clear saves nothing, so the headline named the wrong action. Each
// clear now heads the block with its own failure, and a failed save
// keeps "Couldn't save.".

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig } from './home-handlers';

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
import { m } from '../../src/lib/paraglide/messages';

const APP_INFO = {
	version: '0.12.0',
	history_path: '/home/u/.local/state/ani-gui/history',
	proxy_base_url: 'http://127.0.0.1:31337',
	removed_legacy_paths: []
};
const refused = () => HttpResponse.json({ key: 'error.io.generic', kind: 'io' }, { status: 500 });

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/app-info`, () => HttpResponse.json(APP_INFO)),
		http.delete(`${API_BASE}/api/history`, refused),
		http.delete(`${API_BASE}/api/cache/images`, refused)
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 4000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const button = (label: string) =>
	(Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(label)) as
		| HTMLButtonElement
		| undefined) ?? null;
const alertHeadline = () =>
	target.querySelector('[role="alert"] .state-headline')?.textContent?.trim() ?? null;

describe('settings — a failed clear', () => {
	for (const [name, label, headline] of [
		['history', () => m.settings_clear_history_button(), () => m.settings_clear_history_failed()],
		['image cache', () => m.settings_clear_images_button(), () => m.settings_clear_images_failed()]
	] as const) {
		it(`heads the error with the ${name} clear, not with a save`, async () => {
			app = mount(SettingsPage, { target });
			await until(() => button(label()) !== null, `the clear ${name} button`);
			button(label())!.click();
			await until(() => alertHeadline() !== null, 'the error block');
			expect(alertHeadline()).toBe(headline());
			expect(alertHeadline()).not.toBe(m.settings_error_save_headline());
		});
	}

	it('takes a clear’s failure down when a later clear lands', async () => {
		let failing = true;
		server.use(
			http.delete(`${API_BASE}/api/history`, () =>
				failing ? refused() : new HttpResponse(null, { status: 204 })
			)
		);
		app = mount(SettingsPage, { target });
		const label = () => m.settings_clear_history_button();
		await until(() => button(label()) !== null, 'the clear history button');
		button(label())!.click();
		await until(() => alertHeadline() !== null, 'the error block');
		failing = false;
		await until(() => button(label()) !== null && !button(label())!.disabled, 'the button back');
		button(label())!.click();
		await until(
			() => button(m.settings_clear_history_cleared()) !== null,
			'the cleared confirmation'
		);
		expect(alertHeadline()).toBeNull();
	});

	it('heads a failed save with the save, after a failed clear', async () => {
		server.use(http.put(`${API_BASE}/api/settings`, refused));
		app = mount(SettingsPage, { target });
		await until(() => button(m.settings_clear_history_button()) !== null, 'the page');
		button(m.settings_clear_history_button())!.click();
		await until(() => alertHeadline() === m.settings_clear_history_failed(), 'the clear error');
		const toggle = target.querySelector(
			`input[aria-label="${m.settings_cache_resolutions_aria_label()}"]`
		) as HTMLInputElement;
		toggle.click();
		await until(() => alertHeadline() === m.settings_error_save_headline(), 'the save error');
	});
});
