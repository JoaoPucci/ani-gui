// Acceptance: leaving a show ends its franchise read and its first hop.
//
// The franchise route holds its response open while the backend walks
// the franchise's relation listings, several rounds of Kitsu reads
// when Kitsu is slow. Chromium opens at most six connections to one
// origin over plain HTTP and queues every request past the sixth. The
// franchise read and the walk's first hop, which the page reads beside
// it to hold the strip's room, go to the franchise lane, an origin of
// their own. Reads the page kept after the viewer moved on would hold
// that lane's six for nothing: a few shows browsed while Kitsu is
// slow, and the next show's own franchise read would wait behind them.
//
// jsdom has no connection limit, so this scenario puts Chromium's in
// front of `fetch` (`./connections`).

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { cancelledUnder, sixConnections, type Connections } from './connections';

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

const FRANCHISE = '/api/kitsu/franchise/';

/** The franchise lane's origin, as the backend announces it. */
const FRANCHISE_LANE = 'http://127.0.0.1:31351';
type Bridge = { apiBase: string; lanes?: Record<string, string> };
const bridge = () => (window as unknown as { aniGui: Bridge }).aniGui;

/** The shows whose franchise read reached the franchise lane. */
const reached: string[] = [];
const HOP = '/first-hop';
/** The shows whose franchise walk the page cancelled. */
const walks = (c: Connections) => cancelledUnder(c, FRANCHISE).filter((p) => !p.endsWith(HOP));
/** The shows whose first hop the page cancelled. */
const hops = (c: Connections) =>
	cancelledUnder(c, FRANCHISE)
		.filter((p) => p.endsWith(HOP))
		.map((p) => p.slice(0, -HOP.length));

/** Franchise reads still held open, ended when the scenario ends. */
const holding: Array<() => void> = [];

/** Every show answers at once, except its franchise: Kitsu is slow. */
function slowKitsu() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json({
				...kitsuRef(String(params.id), `Show ${String(params.id)}`, 12),
				subtype: 'TV'
			})
		),
		http.get(`${FRANCHISE_LANE}/api/kitsu/franchise/:id`, ({ params }) => {
			reached.push(String(params.id));
			return new Promise<Response>((resolve) =>
				holding.push(() => resolve(new HttpResponse(null, { status: 503 })))
			);
		}),
		// The walk's first hop waits on Kitsu too.
		http.get(
			`${FRANCHISE_LANE}/api/kitsu/franchise/:id/first-hop`,
			() =>
				new Promise<Response>((resolve) =>
					holding.push(() => resolve(new HttpResponse(null, { status: 503 })))
				)
		),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let pool: Connections | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	bridge().lanes = { franchise: FRANCHISE_LANE };
	reached.length = 0;
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	pool?.restore();
	pool = null;
	for (const end of holding.splice(0)) end();
	delete bridge().lanes;
	target.remove();
});

const title = () => target.querySelector('h1.title')?.textContent ?? '';

async function until(predicate: () => boolean, what: string, timeoutMs = 3000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

describe('detail route — leaving a show while its franchise is read', () => {
	it("cancels the show's franchise read, so the next show's own read is not held behind it", async () => {
		slowKitsu();
		pool = sixConnections();
		setParams({ id: '101' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 101', 'show 101');
		// Six shows, each left while its franchise is still being read.
		for (const id of ['102', '103', '104', '105', '106']) {
			setParams({ id });
			await until(() => title() === `Show ${id}`, `show ${id}`);
		}
		expect(walks(pool)).toEqual(['101', '102', '103', '104', '105']);
		expect(hops(pool)).toEqual(['101', '102', '103', '104', '105']);
		// The seventh show's details come through, and its own franchise
		// read reaches the lane rather than waiting behind the others.
		setParams({ id: '107' });
		await until(() => title() === 'Show 107', 'show 107');
		await until(() => reached.includes('107'), "show 107's franchise read");
	});

	it('cancels the franchise read when the page itself goes', async () => {
		slowKitsu();
		pool = sixConnections();
		setParams({ id: '201' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 201', 'show 201');
		unmount(app);
		app = null;
		expect(walks(pool)).toEqual(['201']);
		expect(hops(pool)).toEqual(['201']);
	});
});
