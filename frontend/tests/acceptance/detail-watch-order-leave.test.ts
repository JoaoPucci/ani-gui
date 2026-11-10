// Acceptance: leaving a show ends its franchise read.
//
// The franchise route holds its response open while the backend walks
// the franchise's relation listings, several rounds of Kitsu reads
// when Kitsu is slow. Chromium opens at most six connections to one
// origin over plain HTTP and queues every request past the sixth, and
// the franchise read shares the API lane with the show page's own
// details, the settings and the history. A read the page kept after
// the viewer moved on would hold one of those six for nothing: six
// shows browsed while Kitsu is slow, and the seventh show's details
// would wait behind them.
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
		http.get(
			`${API_BASE}/api/kitsu/franchise/:id`,
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
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	pool?.restore();
	pool = null;
	for (const end of holding.splice(0)) end();
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
	it("cancels the show's franchise read, so the next show's details are not held behind it", async () => {
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
		expect(cancelledUnder(pool, FRANCHISE)).toEqual(['101', '102', '103', '104', '105']);
		// The seventh show's details come through.
		setParams({ id: '107' });
		await until(() => title() === 'Show 107', 'show 107');
	});

	it('cancels the franchise read when the page itself goes', async () => {
		slowKitsu();
		pool = sixConnections();
		setParams({ id: '201' });
		app = mount(DetailPage, { target });
		await until(() => title() === 'Show 201', 'show 201');
		unmount(app);
		app = null;
		expect(cancelledUnder(pool, FRANCHISE)).toEqual(['201']);
	});
});
