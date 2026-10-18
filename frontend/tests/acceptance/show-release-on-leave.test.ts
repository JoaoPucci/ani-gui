// Acceptance: leaving a show releases that show's warmed plays.
//
// The detail and player pages warm plays for the show on screen, and
// cancel them when the viewer moves on, so a resolve nobody will use
// stops holding the provider's slots. The cleanup has to cancel the
// show the page was showing. By the time it runs the route already
// names where the viewer went: home names no show at all, and another
// show's page names that show. Read from the route at that moment,
// the cleanup released nothing, or released the new show's plays,
// while the old show's kept resolving. And the detail page is kept
// across a move to another show, so it has to release every show it
// showed, not only the last.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { flushSync, mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests, getOrFire, makeKey } from '../../src/lib/play/play-cache';

const SHOW = '42';
const OTHER = '77';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	target = document.createElement('div');
	document.body.appendChild(target);
	useShowHandlers();
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

/** Both shows answer the same way, so whichever the route names can
 *  load. */
function useShowHandlers() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json({
				...kitsuRef(String(params.id), `Show ${String(params.id)}`, 12),
				status: 'finished'
			})
		),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
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
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

/** The warm streams opened for `show`, by the title the fixture gives it. */
const streamsFor = (show: string) =>
	FakeEventSource.instances.filter(
		(es) => new URL(es.url).searchParams.get('title') === `Show ${show}`
	);

describe("leaving a show's detail page", () => {
	it('for home, cancels the plays it warmed for the show', async () => {
		setParams({ id: SHOW });
		app = mount(DetailPage, { target });
		await until(() => streamsFor(SHOW).length > 0, "the page's warm for its show");

		// Home names no show: the route moves on before the page goes.
		setParams({});
		flushSync();
		unmount(app);
		app = null;

		expect(streamsFor(SHOW).every((es) => es.closed)).toBe(true);
	});

	it('after moving to another show, cancels the plays it warmed for both', async () => {
		setParams({ id: SHOW });
		app = mount(DetailPage, { target });
		await until(() => streamsFor(SHOW).length > 0, "the page's warm for its show");

		// The route keeps the component and hands it the other show.
		// The first show's warms carry on across the move: they are
		// still that show's, and a play clicked there still answers.
		setParams({ id: OTHER });
		flushSync();
		await until(() => streamsFor(OTHER).length > 0, "the page's warm for the other show");

		setParams({});
		flushSync();
		unmount(app);
		app = null;

		expect(streamsFor(SHOW).every((es) => es.closed)).toBe(true);
		expect(streamsFor(OTHER).every((es) => es.closed)).toBe(true);
	});
});

describe('leaving the player', () => {
	it("for another show's page, cancels the played show's plays, not that show's", async () => {
		setParams({ id: SHOW });
		setUrl(`/play/${SHOW}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => streamsFor(SHOW).length > 0, "the player's next-episode warm");

		// The other show's page is already warming its own play when
		// the player goes.
		const othersKey = makeKey(OTHER, 1, 'sub', 'best');
		const othersWarm = getOrFire(othersKey, () => new Promise(() => {}));
		// No flush between the route change and the unmount: SvelteKit
		// destroys the leaving page's branch before that page's effects
		// would see the new route, so the player never reports the
		// other show as its own. The detail cases flush because there
		// the component is kept and does see it.
		setParams({ id: OTHER });
		unmount(app);
		app = null;

		expect(streamsFor(SHOW).every((es) => es.closed)).toBe(true);
		// Still the same entry: the other show's warm was left alone.
		expect(getOrFire(othersKey, () => new Promise(() => {}))).toBe(othersWarm);
	});
});
