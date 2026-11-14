// Acceptance: leaving Home cancels the reads it started.
//
// Home reads its two rows of shows, the Watch Later list, and, for each
// Continue Watching card, the reads that match its history row to a
// Kitsu entry: the stored mapping, the remembered title match, a Kitsu
// search. All of them go to the API lane, where Chromium opens at most
// six connections and queues the rest, and Home cancelled none of them
// when it was left: a viewer who opened Home while Kitsu was slow and
// moved on left five reads holding connections, and two such visits
// left the next page's reads queued behind them.
//
// jsdom has no connection limit, so this scenario puts one in front of
// `fetch` (`./connections`).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers } from './home-handlers';
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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { accountStore } from '../../src/lib/account/store.svelte';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	localStorage.clear();
	reads = hangingReads();
	// A tracker is connected, so Home reads the Watch Later list.
	accountStore.setConnected('anilist', {
		access_token: 'test-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'user-1',
		username: 'tester',
		avatar_url: null
	});
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	pool?.restore();
	pool = null;
	reads.release();
	accountStore.setDisconnected('anilist');
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

/** Two history rows with no stored mapping and no remembered title,
 *  so each card searches Kitsu for its show. */
const HISTORY = [
	{ id: 'hianime:first-show-1', ep_no: '2', title: 'First Show' },
	{ id: 'hianime:second-show-2', ep_no: '5', title: 'Second Show' }
];

/** The five reads one visit holds: both rows, the Watch Later list and
 *  each card's search. */
const PER_VISIT = 5;

describe('home route — leaving while its reads are out', () => {
	it('cancels every read still out when Home is left, so the next visit comes through', async () => {
		server.use(
			...homeHandlers({ history: HISTORY }, [
				http.get(`${API_BASE}/api/kitsu/trending-anilist`, reads.hang),
				http.get(`${API_BASE}/api/kitsu/top-rated`, reads.hang),
				http.get(`${API_BASE}/api/account/list/:provider/cached`, reads.hang),
				http.post(`${API_BASE}/api/kitsu/search`, reads.hang)
			])
		);
		pool = sixConnections();
		// Two visits, each left while its reads are held: more than six
		// connections' worth of reads nobody would read.
		for (const visit of [1, 2]) {
			app = mount(HomePage, { target });
			await until(() => reads.held.length >= visit * PER_VISIT, `visit ${visit}'s reads`);
			unmount(app);
			app = null;
			expect(notCancelled(pool, heldPaths(reads))).toEqual([]);
		}
		// The third visit's reads get connections only if the first two
		// gave theirs back.
		app = mount(HomePage, { target });
		await until(() => reads.held.length >= 3 * PER_VISIT, "the third visit's reads");
	});
});
