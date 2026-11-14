// Acceptance: leaving the search page cancels the reads it started.
//
// The search page reads the trending and top-rated rows and the page of
// Kitsu's results the search asked for, each on the API lane, where
// Chromium opens at most six connections and queues the rest. The page
// cancelled none of them when it went: a search left while Kitsu was
// slow kept its reads out, and two such visits left the next page's
// reads queued behind six nobody would read.
//
// jsdom has no connection limit, so this scenario puts one in front of
// `fetch` (`./connections`).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setQuery } from './page-state.svelte';
import { appConfig } from './home-handlers';
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

import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	reads = hangingReads();
	setQuery('frieren');
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

/** The three reads one visit holds: both rows and the results page. */
const PER_VISIT = 3;

describe('search route — leaving while its reads are out', () => {
	it('cancels every read still out when the page goes, so the next visit comes through', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/trending`, reads.hang),
			http.get(`${API_BASE}/api/kitsu/top-rated`, reads.hang),
			http.post(`${API_BASE}/api/kitsu/search/page`, reads.hang)
		);
		pool = sixConnections();
		// Two visits, each left while its reads are held: six
		// connections' worth of reads nobody would read.
		for (const visit of [1, 2]) {
			app = mount(SearchPage, { target });
			await until(() => reads.held.length >= visit * PER_VISIT, `visit ${visit}'s reads`);
			unmount(app);
			app = null;
			expect(notCancelled(pool, heldPaths(reads))).toEqual([]);
		}
		// The third visit's reads get connections only if the first two
		// gave theirs back.
		app = mount(SearchPage, { target });
		await until(() => reads.held.length >= 3 * PER_VISIT, "the third visit's reads");
	});
});
