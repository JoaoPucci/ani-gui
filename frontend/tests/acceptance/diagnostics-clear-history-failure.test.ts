// Acceptance: a failed Clear history on Diagnostics says the clear
// failed, and keeps the history list it had loaded.
//
// The clear's failure went into the load's error, so the section read
// "Couldn't load history." and the list that had loaded was replaced
// by it.

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
import { m } from '../../src/lib/paraglide/messages';

const ROW = { ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' };
let failing = true;
let listFails = false;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	failing = true;
	listFails = false;
	server.use(
		http.get(`${API_BASE}/api/app-info`, () =>
			HttpResponse.json({
				version: '0.15.1',
				history_path: '/home/u/.local/state/ani-gui/history',
				proxy_base_url: API_BASE,
				removed_legacy_paths: []
			})
		),
		http.get(`${API_BASE}/api/history`, () =>
			listFails
				? HttpResponse.json({ key: 'error.io.generic', kind: 'io' }, { status: 500 })
				: HttpResponse.json([ROW])
		),
		http.delete(`${API_BASE}/api/history`, () =>
			failing
				? HttpResponse.json({ key: 'error.io.generic', kind: 'io' }, { status: 500 })
				: new HttpResponse(null, { status: 204 })
		)
	);
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 4000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const clearButton = () =>
	(Array.from(target.querySelectorAll('button')).find(
		(b) => b.textContent?.trim() === m.diagnostics_clear_history_button()
	) as HTMLButtonElement | undefined) ?? null;
const listed = () => (target.textContent ?? '').includes(ROW.id);
const text = () => target.textContent ?? '';

describe('diagnostics — a failed Clear history', () => {
	it('says the clear failed, not the load, and keeps the list', async () => {
		app = mount(DiagnosticsPage, { target });
		await until(() => listed() && clearButton() !== null, 'the history list');
		clearButton()!.click();
		await until(() => text().includes(m.diagnostics_history_clear_error()), 'the clear error');
		expect(text()).not.toContain(m.diagnostics_history_error());
		expect(listed()).toBe(true);
	});

	it('takes the failure down when a later clear lands', async () => {
		app = mount(DiagnosticsPage, { target });
		await until(() => listed() && clearButton() !== null, 'the history list');
		clearButton()!.click();
		await until(() => text().includes(m.diagnostics_history_clear_error()), 'the clear error');
		failing = false;
		clearButton()!.click();
		await until(() => !listed(), 'the list to empty');
		expect(text()).not.toContain(m.diagnostics_history_clear_error());
	});

	const refreshButton = () =>
		(Array.from(target.querySelectorAll('button')).find(
			(b) => b.textContent?.trim() === m.diagnostics_refresh_button()
		) as HTMLButtonElement | undefined) ?? null;

	it('takes the clear’s failure down when a refresh loads the list', async () => {
		app = mount(DiagnosticsPage, { target });
		await until(() => listed() && clearButton() !== null, 'the history list');
		clearButton()!.click();
		await until(() => text().includes(m.diagnostics_history_clear_error()), 'the clear error');
		await until(() => !refreshButton()!.disabled, 'the buttons back');
		refreshButton()!.click();
		await until(() => !text().includes(m.diagnostics_history_clear_error()), 'the error to go');
		expect(listed()).toBe(true);
	});

	it('shows the load’s error, not the clear’s, when the refresh fails', async () => {
		app = mount(DiagnosticsPage, { target });
		await until(() => listed() && clearButton() !== null, 'the history list');
		clearButton()!.click();
		await until(() => text().includes(m.diagnostics_history_clear_error()), 'the clear error');
		await until(() => !refreshButton()!.disabled, 'the buttons back');
		listFails = true;
		refreshButton()!.click();
		await until(() => text().includes(m.diagnostics_history_error()), 'the load error');
		expect(text()).not.toContain(m.diagnostics_history_clear_error());
	});
});
