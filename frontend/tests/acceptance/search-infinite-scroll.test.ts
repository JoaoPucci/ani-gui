// Acceptance: /search pages through Kitsu as the reader reaches the
// bottom of the grid.
//
// Kitsu ranks a text search by popularity and serves twenty rows a
// page, so a minor show sits pages deep: "Dragon Ball Super: Beerus"
// is the 68th hit for "dragon ball". These scenarios drive the real
// route, runner, paging rules and availability filter; MSW plays the
// backend, and a controllable IntersectionObserver plays the scroll —
// happy-dom has no layout, so nothing would ever intersect on its own.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setQuery } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { subtypeLabel } from '../../src/lib/search/subtype-label';

function ref(id: string, title: string) {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: 12,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null
	};
}

const CONFIG = {
	locale: 'en',
	mode: 'sub',
	quality: 'best',
	external_player: '',
	external_player_kind: 'mpv',
	external_player_custom_args: '',
	syncplay_binary: '',
	image_cache_cap_mb: 100,
	auto_play_next: false,
	download_bottom_bar_enabled: true,
	auto_skip_op: false,
	auto_skip_ed: false,
	use_custom_player_controls: true,
	auto_update_anicli: false,
	update_include_prereleases: false,
	primary_account: ''
};

/** Every id already known playable, so no probe runs. */
function baseHandlers() {
	return [
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(CONFIG)),
		http.get(`${API_BASE}/api/kitsu/trending`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const { kitsu_ids } = (await request.json()) as { kitsu_ids: string[] };
			return HttpResponse.json({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		})
	];
}

/** The scroll stand-in: every observer the route creates, and a way to
 *  bring their targets into reach. */
const observers: { cb: IntersectionObserverCallback; targets: Element[] }[] = [];
class ControlledObserver {
	private entry: { cb: IntersectionObserverCallback; targets: Element[] };
	constructor(cb: IntersectionObserverCallback) {
		this.entry = { cb, targets: [] };
		observers.push(this.entry);
	}
	observe(el: Element) {
		this.entry.targets.push(el);
	}
	disconnect() {
		const i = observers.indexOf(this.entry);
		if (i >= 0) observers.splice(i, 1);
	}
}

function reachBottom(inReach = true) {
	for (const o of observers.slice()) {
		o.cb(
			o.targets.map((target) => ({ target, isIntersecting: inReach }) as IntersectionObserverEntry),
			{} as IntersectionObserver
		);
	}
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let savedObserver: unknown;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	savedObserver = globalThis.IntersectionObserver;
	globalThis.IntersectionObserver = ControlledObserver as unknown as typeof IntersectionObserver;
	observers.length = 0;
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	globalThis.IntersectionObserver = savedObserver as typeof IntersectionObserver;
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const screen = () => target.textContent ?? '';

function buttonNamed(name: string): HTMLButtonElement | null {
	return [...target.querySelectorAll('button')].find((b) => b.textContent?.trim() === name) ?? null;
}

describe('/search infinite scroll', () => {
	it('loads the next page when the reader reaches the bottom, showing that it is loading', async () => {
		let releaseSecond: () => void = () => {};
		const secondHeld = new Promise<void>((r) => (releaseSecond = r));
		const offsets: number[] = [];
		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search/page`, async ({ request }) => {
				const { offset } = (await request.json()) as { offset: number };
				offsets.push(offset);
				if (offset === 0) {
					return HttpResponse.json({ hits: [ref('1', 'Dragon Ball Z')], next_offset: 20 });
				}
				await secondHeld;
				return HttpResponse.json({
					hits: [ref('68', 'Dragon Ball Super: Beerus')],
					next_offset: 40
				});
			})
		);

		setQuery('dragon ball');
		app = mount(SearchPage, { target });
		await until(() => screen().includes('Dragon Ball Z'), 'the first page');
		expect(offsets).toEqual([0]);
		expect(screen()).not.toContain(m.search_paging_loading());

		reachBottom();
		await until(() => screen().includes(m.search_paging_loading()), 'the loading indicator');
		expect(offsets).toEqual([0, 20]);
		// Announced politely, not as an alert.
		const live = target.querySelector('[aria-live="polite"][data-testid="search-paging"]');
		expect(live?.textContent).toContain(m.search_paging_loading());

		releaseSecond();
		await until(() => screen().includes('Dragon Ball Super: Beerus'), 'the second page');
		expect(screen()).toContain('Dragon Ball Z');
		await until(
			() => !screen().includes(m.search_paging_loading()),
			'the indicator to clear once the page lands'
		);
	});

	it('says the results have ended once Kitsu has no more pages', async () => {
		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search/page`, async ({ request }) => {
				const { offset } = (await request.json()) as { offset: number };
				if (offset === 0) {
					return HttpResponse.json({ hits: [ref('1', 'Trigun')], next_offset: 20 });
				}
				return HttpResponse.json({ hits: [ref('2', 'Trigun Stampede')], next_offset: null });
			})
		);

		setQuery('trigun');
		app = mount(SearchPage, { target });
		await until(() => screen().includes('Trigun'), 'the first page');
		expect(screen()).not.toContain(m.search_paging_end({ query: 'trigun' }));

		reachBottom();
		await until(
			() => screen().includes(m.search_paging_end({ query: 'trigun' })),
			'the end-of-results line'
		);
		expect(screen()).toContain('Trigun Stampede');
	});

	it('a page that fails offers a retry that loads it', async () => {
		let failing = true;
		const FAILURE = { kind: 'upstream', key: 'error.network.upstream', detail: 'upstream 503' };
		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search/page`, async ({ request }) => {
				const { offset } = (await request.json()) as { offset: number };
				if (offset === 0) {
					return HttpResponse.json({ hits: [ref('1', 'Cowboy Bebop')], next_offset: 20 });
				}
				if (failing) return HttpResponse.json(FAILURE, { status: 502 });
				return HttpResponse.json({
					hits: [ref('2', 'Cowboy Bebop: The Movie')],
					next_offset: null
				});
			})
		);

		setQuery('cowboy');
		app = mount(SearchPage, { target });
		await until(() => screen().includes('Cowboy Bebop'), 'the first page');

		reachBottom();
		await until(() => screen().includes(m.search_paging_failed()), 'the failed-page line');
		const retry = buttonNamed(m.search_paging_retry());
		expect(retry).not.toBeNull();
		// A real button in the tab order, so the keyboard reaches it.
		expect(retry!.disabled).toBe(false);
		expect(retry!.tabIndex).toBeGreaterThanOrEqual(0);
		// What was already shown stays.
		expect(screen()).toContain('Cowboy Bebop');

		failing = false;
		retry!.click();
		await until(() => screen().includes('Cowboy Bebop: The Movie'), 'the retried page');
		expect(screen()).not.toContain(m.search_paging_failed());
	});

	it('with a type filter on, loads more only when asked', async () => {
		// The filter keeps the grid short, so the bottom stays in reach;
		// loading by itself there would walk the whole search.
		const offsets: number[] = [];
		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search/page`, async ({ request }) => {
				const { offset } = (await request.json()) as { offset: number };
				offsets.push(offset);
				if (offset === 0) {
					return HttpResponse.json({
						hits: [
							{ ...ref('1', 'Dragon Ball Z'), subtype: 'TV' },
							{ ...ref('2', 'Dragon Ball: Path to Power'), subtype: 'movie' }
						],
						next_offset: 20
					});
				}
				return HttpResponse.json({
					hits: [{ ...ref('3', 'Dragon Ball Super: Broly'), subtype: 'movie' }],
					next_offset: null
				});
			})
		);

		setQuery('dragon ball');
		app = mount(SearchPage, { target });
		await until(() => screen().includes('Dragon Ball Z'), 'the first page');
		buttonNamed(subtypeLabel('movie'))!.click();
		await until(() => !screen().includes('Dragon Ball Z'), 'the movie filter to apply');

		reachBottom();
		// Let any effect the report sets off run, and its request leave.
		for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 20));
		expect(offsets).toEqual([0]);

		const loadMore = buttonNamed(m.search_paging_load_more());
		expect(loadMore).not.toBeNull();
		loadMore!.click();
		await until(() => screen().includes('Dragon Ball Super: Broly'), 'the asked-for page');
		expect(offsets).toEqual([0, 20]);
	});
});
