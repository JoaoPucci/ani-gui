// Acceptance: leaving a page cancels the reads its show question
// started.
//
// A page a guessed Continue card opened — the player, the show page —
// reads the history and the card row's match to decide whether to ask
// "Is this the show you meant?", and Home and the show page read the
// history again whenever a link answers. Each of those reads holds one
// of the API lane's six connections until it answers, and none was
// cancelled when the page went: a viewer who left while the database
// or Kitsu was slow left them holding connections the next page's reads
// queued behind ($lib/navigation/visit).
//
// jsdom has no connection limit, so these scenarios put one in front of
// `fetch` (`./connections`) and check the held reads were cancelled.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { sixConnections, type Connections } from './connections';
import { hangingReads, type HangingReads } from './hanging-reads';

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
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { appLinkDeps } from '../../src/lib/history/link-deps';

const KITSU_ID = '42';
const CARD = 'hianime:the-show-2000-5';

/** A stream that never resolves: the scenarios are about the reads
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

async function until(predicate: () => boolean, what: string, timeoutMs = 4000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** The show answers, and everything else the page reads beside the
 *  history answers at once with nothing. */
function showReads() {
	server.use(
		http.get(`${API_BASE}/api/kitsu/anime/:id`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'The Show', 12), status: 'finished' })
		),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(null, { status: 503 })),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/history/link`, () =>
			HttpResponse.json({ kitsu_id: KITSU_ID, linked: [] })
		)
	);
}

const heldUnder = (path: string) => reads.held.filter((h) => h.endsWith(` ${path}`)).length;
const cancelledUnder = (path: string) => pool!.cancelled.filter((p) => p === path).length;

/** Waits for the page's held reads of `path`, leaves the page, and
 *  checks every one of them was cancelled as it went. */
async function leaveWhileOut(path: string) {
	await until(() => heldUnder(path) > 0, `the read of ${path} to be out`);
	unmount(app!);
	app = null;
	expect(cancelledUnder(path)).toBe(heldUnder(path));
}

describe("the show question's reads", () => {
	it("are cancelled when the player of a guessed card's session is left", async () => {
		showReads();
		server.use(http.get(`${API_BASE}/api/history`, reads.hang));
		pool = sixConnections();
		setParams({ id: KITSU_ID });
		setUrl(`/play/${KITSU_ID}`, { episode: '1', guess: '1', row: CARD });
		app = mount(PlayPage, { target });
		await leaveWhileOut('/api/history');
	});

	it('are cancelled when the show page a guessed card opened is left', async () => {
		showReads();
		server.use(http.get(`${API_BASE}/api/history`, reads.hang));
		pool = sixConnections();
		setParams({ id: KITSU_ID });
		setUrl(`/anime/${KITSU_ID}`, { guessed: CARD });
		app = mount(DetailPage, { target });
		await leaveWhileOut('/api/history');
	});
});

describe('the reads a link answered starts', () => {
	it("are cancelled when the show page is left: its resume row's re-read", async () => {
		showReads();
		pool = sixConnections();
		setParams({ id: KITSU_ID });
		setUrl(`/anime/${KITSU_ID}`);
		app = mount(DetailPage, { target });
		await until(
			() => (target.querySelector('h1.title')?.textContent ?? '').includes('The Show'),
			'the show page'
		);
		// The page's own resume read has answered; the one a link starts
		// is held.
		await new Promise((r) => setTimeout(r, 100));
		server.use(http.get(`${API_BASE}/api/history/by-kitsu/:id`, reads.hang));
		await appLinkDeps().link(CARD, [CARD], KITSU_ID);
		await leaveWhileOut(`/api/history/by-kitsu/${KITSU_ID}`);
	});

	it("are cancelled when Home is left: the cards' re-read", async () => {
		server.use(
			...homeHandlers({ history: [{ id: CARD, ep_no: '2', title: 'The Show' }] }),
			http.post(`${API_BASE}/api/history/link`, () =>
				HttpResponse.json({ kitsu_id: KITSU_ID, linked: [] })
			)
		);
		pool = sixConnections();
		setUrl('/');
		app = mount(HomePage, { target });
		await until(() => target.querySelector('.resume-cell') !== null, "Home's Continue cards");
		server.use(http.get(`${API_BASE}/api/history`, reads.hang));
		await appLinkDeps().link(CARD, [CARD], KITSU_ID);
		await leaveWhileOut('/api/history');
	});
});
