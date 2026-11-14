// Acceptance: leaving a show cancels the reads its page started.
//
// Every read the show page sends goes to the API lane, and Chromium
// opens at most six connections to one origin over plain HTTP and
// queues every request past the sixth. The page answered from its
// reads and never cancelled one, so a show left while Kitsu, AniList or
// a tracker was slow kept its similar titles, its airing schedule, its
// episodes, its resume row, its list entry, the settings and the
// download folder out, each holding a connection for an answer nobody
// would read. A viewer browsing a few shows while Kitsu was slow found
// the next show's details queued behind them.
//
// jsdom has no connection limit, so this scenario puts one in front of
// `fetch` (`./connections`): six requests in flight to the API's
// origin, the rest queued until one answers, fails or is cancelled.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;
let reads: HangingReads;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	reads = hangingReads();
	// A tracker is connected, so the page reads the show's list entry.
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

/** Each show's details come at once and the provider lists it; every
 *  other read the page sends is held, as a slow Kitsu, AniList, tracker
 *  and database hold them. The show airs, so its schedule is read. */
function slowShowReads(held: HangingReads) {
	server.use(
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json({
				...kitsuRef(String(params.id), `Show ${String(params.id)}`, 12),
				status: 'current',
				subtype: 'TV'
			})
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
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, held.hang),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, held.hang),
		http.get(`${API_BASE}/api/account/entry/:provider`, held.hang)
	);
}

/** The two reads the page makes once, whichever show it shows. */
const ONCE = new Set(['/api/settings', '/api/download/default-dir']);

/** What `held` holds for the shows, not the page: every read but the
 *  settings and the download folder. */
const showReads = (held: string[]) => held.filter((p) => !ONCE.has(p));

const title = () => target.querySelector('h1.title')?.textContent ?? '';

async function until(predicate: () => boolean, what: string, timeoutMs = 4000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** What `paths` still has that was not cancelled, for a failure message. */
function notCancelled(c: Connections, paths: string[]): string[] {
	const left = [...c.cancelled];
	return paths.filter((p) => {
		const at = left.indexOf(p);
		if (at < 0) return true;
		left.splice(at, 1);
		return false;
	});
}

/** Waits until `more` reads beyond the `since` already held are out:
 *  with six connections, the page has filled its share of them. */
const heldBeyond = (since: number, more: number) =>
	until(() => reads.held.length >= since + more, `${more} more reads to be out`);

describe('detail route — leaving a show while its reads are out', () => {
	it('cancels every read of the show left, so the next show comes through', async () => {
		slowShowReads(reads);
		pool = sixConnections();
		setParams({ id: '101' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 101', 'show 101');
		// Every connection is taken by a read nobody has answered: the
		// settings, the download folder and the show's own.
		await heldBeyond(0, 6);
		// The next show's details get a connection only if the first
		// show's reads gave theirs back.
		setParams({ id: '102' });
		await until(() => title() === 'Show 102', 'show 102');
		// Its reads take the connections the first show's gave back.
		await heldBeyond(6, 4);
		// Everything held so far belongs to the two shows left, but for
		// the page's own two reads.
		const left = showReads(heldPaths(reads));
		setParams({ id: '103' });
		await until(() => title() === 'Show 103', 'show 103');
		// Every read held for a show left was cancelled.
		expect(left).toContain('/api/history/by-kitsu/101');
		expect(left).toContain('/api/history/by-kitsu/102');
		expect(notCancelled(pool, left)).toEqual([]);
		// The page's own reads and the show on screen keep theirs.
		expect(pool.cancelled.some((p) => ONCE.has(p) || p.endsWith('/103'))).toBe(false);
	});

	it('cancels every read still out when the page itself goes', async () => {
		slowShowReads(reads);
		pool = sixConnections();
		setParams({ id: '201' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 201', 'show 201');
		await heldBeyond(0, 6);
		unmount(app);
		app = null;
		expect(notCancelled(pool, heldPaths(reads))).toEqual([]);
	});

	it('shows no failure on the show moved to for the cancelled reads of the show left', async () => {
		slowShowReads(reads);
		pool = sixConnections();
		setParams({ id: '301' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 301', 'show 301');
		await heldBeyond(0, 6);
		const left = showReads(heldPaths(reads));
		setParams({ id: '302' });
		await until(() => title() === 'Show 302', 'show 302');
		await until(() => notCancelled(pool!, left).length === 0, "show 301's reads to be cancelled");
		await new Promise((r) => setTimeout(r, 50));
		// Show 302's own reads are still out: its episodes are loading,
		// not unavailable.
		const text = target.textContent ?? '';
		expect(text).not.toContain(m.detail_episodes_error_message());
		expect(text).not.toContain(m.detail_episodes_range_unavailable());
		expect(text).toContain(m.detail_episodes_range_loading());
	});
});
