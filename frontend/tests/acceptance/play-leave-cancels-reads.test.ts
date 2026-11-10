// Acceptance: leaving the player cancels the reads its page started.
//
// Beside the stream, the player reads the show's details and similar
// titles, the settings, the download folder, the strip's episodes and
// the airing schedule, all on the API lane, where Chromium opens at
// most six connections and queues the rest. The page cancelled none of
// them when it went: a viewer who opened two episodes while Kitsu was
// slow and left both left ten reads holding connections, and the next
// page's own reads queued behind them.
//
// jsdom has no connection limit, so this scenario puts one in front of
// `fetch` (`./connections`).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef } from './home-handlers';
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

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';

/** A stream that never resolves: the scenario is about the reads
 *  beside it. */
class SilentEventSource {
	addEventListener() {}
	close() {}
}
type GlobalLike = { EventSource?: unknown };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	localStorage.clear();
	g.EventSource = SilentEventSource;
	reads = hangingReads();
	setParams({ id: KITSU_ID });
	setUrl(`/play/${KITSU_ID}`, { episode: '1' });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	pool?.restore();
	pool = null;
	reads.release();
	delete g.EventSource;
	target.remove();
	localStorage.clear();
});

/** The show's details come at once and the provider lists it; every
 *  other read is held, as a slow Kitsu, AniList and database hold
 *  them. The show airs, so its schedule is read. */
function slowReads(held: HangingReads) {
	server.use(
		http.get(`${API_BASE}/api/kitsu/anime/:id`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Ongoing Show', 12), status: 'current' })
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/settings`, held.hang),
		http.get(`${API_BASE}/api/download/default-dir`, held.hang),
		http.post(`${API_BASE}/api/kitsu/search`, held.hang),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, held.hang),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, held.hang)
	);
}

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

/** The five reads one player holds while the backend is slow. */
const PER_PAGE = 5;

describe('play route — leaving while its reads are out', () => {
	it('cancels every read still out when the page goes, so the next page comes through', async () => {
		slowReads(reads);
		pool = sixConnections();
		// Two players, each left while its reads are held: more than six
		// connections' worth of reads nobody would read.
		for (const visit of [1, 2]) {
			app = mount(PlayPage, { target });
			await until(() => reads.held.length >= visit * PER_PAGE, `player ${visit}'s reads`);
			unmount(app);
			app = null;
			expect(notCancelled(pool, heldPaths(reads))).toEqual([]);
		}
		// The third player's reads get connections only if the first
		// two gave theirs back.
		app = mount(PlayPage, { target });
		await until(() => reads.held.length >= 3 * PER_PAGE, "the third player's reads");
	});

	it('opens the strip for the show it moved to, when the show changes under it', async () => {
		// The first show's episodes are held; the next show's answer.
		slowReads(reads);
		server.use(
			http.get(`${API_BASE}/api/kitsu/episodes/43`, () =>
				HttpResponse.json([
					{
						id: 'ep-43-1',
						number: 1,
						relative_number: 1,
						canonical_title: 'First of the next show',
						season_number: 1,
						length: 24,
						synopsis: null,
						airdate: '2019-01-01',
						thumbnail: null
					}
				])
			)
		);
		pool = sixConnections();
		app = mount(PlayPage, { target });
		await until(
			() => reads.held.some((h) => h.endsWith(`/api/kitsu/episodes/${KITSU_ID}`)),
			"the first show's episodes to be out"
		);
		setParams({ id: '43' });
		setUrl('/play/43', { episode: '1' });
		await until(
			() => pool!.cancelled.includes(`/api/kitsu/episodes/${KITSU_ID}`),
			"the first show's episodes to be cancelled"
		);
		await until(
			() => (target.textContent ?? '').includes('First of the next show'),
			"the next show's strip"
		);
	});
});
