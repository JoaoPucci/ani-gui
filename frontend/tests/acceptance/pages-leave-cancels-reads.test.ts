// Acceptance: leaving Settings, Account or Diagnostics cancels the
// reads the page started.
//
// Each of them reads from the backend when it opens — the settings, the
// app's own details, the watch history — and none cancelled its reads
// when it went. They answer from the local database, which a busy
// backend can hold up; every read left behind holds one of the API
// lane's six connections, and the page opened next queues behind them.
//
// jsdom has no connection limit, so this scenario puts one in front of
// `fetch` (`./connections`): six requests in flight to the API's
// origin, the rest queued until one answers, fails or is cancelled.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, type HttpHandler } from 'msw';
import { mount, unmount, type Component } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { sixConnections, type Connections } from './connections';
import { hangingReads, heldPaths, type HangingReads } from './hanging-reads';

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
import AccountPage from '../../src/routes/account/+page.svelte';
import DiagnosticsPage from '../../src/routes/diagnostics/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	reads = hangingReads();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	pool?.restore();
	pool = null;
	reads.release();
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

/** What of `paths` was not cancelled, repeats counted. */
function notCancelled(c: Connections, paths: string[]): string[] {
	const left = [...c.cancelled];
	return paths.filter((p) => {
		const at = left.indexOf(p);
		if (at < 0) return true;
		left.splice(at, 1);
		return false;
	});
}

/**
 * Opens `Page` and leaves it while its `perPage` reads are held, as
 * many times as it takes to fill six connections, checking each time
 * that the page going cancelled them; then opens it once more, whose
 * reads get connections only if the ones before gave theirs back.
 */
async function leaveWhileHeld(Page: Component, perPage: number) {
	pool = sixConnections();
	const visits = Math.ceil(6 / perPage);
	for (let visit = 1; visit <= visits; visit++) {
		app = mount(Page, { target });
		await until(() => reads.held.length >= visit * perPage, `visit ${visit}'s reads`);
		unmount(app);
		app = null;
		expect(notCancelled(pool, heldPaths(reads))).toEqual([]);
	}
	app = mount(Page, { target });
	await until(() => reads.held.length >= (visits + 1) * perPage, 'the next visit’s reads');
}

const holding = (handlers: (hang: HangingReads['hang']) => HttpHandler[]) =>
	server.use(...handlers(reads.hang));

describe('leaving a page while its reads are out cancels them', () => {
	it('Settings: the settings and the app details', async () => {
		holding((hang) => [
			http.get(`${API_BASE}/api/settings`, hang),
			http.get(`${API_BASE}/api/app-info`, hang)
		]);
		await leaveWhileHeld(SettingsPage, 2);
	});

	it('Account: the settings', async () => {
		holding((hang) => [http.get(`${API_BASE}/api/settings`, hang)]);
		await leaveWhileHeld(AccountPage, 1);
	});

	it('Diagnostics: the app details, then the history', async () => {
		// The page reads the history once its details are in, so the
		// details answer and the history is held.
		holding((hang) => [
			http.get(`${API_BASE}/api/app-info`, () =>
				Response.json({
					version: '0.0.0',
					history_path: '/h',
					proxy_base_url: API_BASE,
					removed_legacy_paths: []
				})
			),
			http.get(`${API_BASE}/api/history`, hang)
		]);
		await leaveWhileHeld(DiagnosticsPage, 1);
	});
});
