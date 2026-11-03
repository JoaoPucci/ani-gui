// Acceptance: going back to the search page restores what the reader
// left — the query, the order, the type filters, the results already
// loaded, and the card that was at the top of the screen.
//
// SvelteKit carries it in a snapshot (https://svelte.dev/docs/kit/snapshots).
// When the reader leaves the search page, SvelteKit calls the page's
// `snapshot.capture` and keeps the value with that history entry,
// writing it to sessionStorage as JSON. On a back or forward press onto
// the entry it mounts the page, calls `snapshot.restore` with the value,
// and then reports the navigation to the page's `afterNavigate` hooks
// as a `popstate`; on a reload it does the same with `enter`. Its own
// scroll restoration runs before `restore`, against a page that has no
// results yet, so the page puts the scroll back itself.
//
// With the router mocked, these scenarios play SvelteKit's part in
// that order. happy-dom has no layout, so each card is given the
// offsets a browser would lay it out at: one card per 300px row.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setQuery, setUrl } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
vi.mock('$app/navigation', () => ({
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import { afterNavigate, beforeNavigate } from '$app/navigation';
import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { m } from '../../src/lib/paraglide/messages';
import { subtypeLabel } from '../../src/lib/search/subtype-label';

const ROW = 300;

/** Show `n` of a search: odd ids are TV series, even ones films. */
function ref(n: number) {
	return {
		id: String(n),
		canonical_title: `Show ${n}`,
		titles: {},
		abbreviated_titles: [],
		slug: `show-${n}`,
		synopsis: 'Not shown in the grid.',
		poster_image: null,
		cover_image: null,
		episode_count: 12,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null,
		subtype: n % 2 === 1 ? 'TV' : 'movie'
	};
}

let mode = 'sub';
const config = () => ({
	locale: 'en',
	mode,
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
	primary_account: '',
	watch_order: 'linked'
});

/** Every search request Kitsu was asked, as `query@offset/sort`. */
const asked: string[] = [];

function handlers() {
	return [
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(config())),
		http.get(`${API_BASE}/api/kitsu/trending`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const { kitsu_ids } = (await request.json()) as { kitsu_ids: string[] };
			return HttpResponse.json({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		}),
		// Twenty shows a page, five pages, for any query.
		http.post(`${API_BASE}/api/kitsu/search/page`, async ({ request }) => {
			const body = (await request.json()) as { query: string; offset: number; sort?: string };
			asked.push(`${body.query}@${body.offset}/${body.sort ?? 'relevance'}`);
			const hits = Array.from({ length: 20 }, (_, i) => ref(body.offset + i + 1));
			return HttpResponse.json({
				hits,
				next_offset: body.offset + 20 < 100 ? body.offset + 20 : null
			});
		})
	];
}

/** An observer that never reports: the scenarios load pages by hand. */
class QuietObserver {
	observe() {}
	unobserve() {}
	disconnect() {}
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let savedObserver: unknown;
let savedOffsetTop: PropertyDescriptor | undefined;
let savedOffsetHeight: PropertyDescriptor | undefined;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	mode = 'sub';
	asked.length = 0;
	server.use(...handlers());
	savedObserver = globalThis.IntersectionObserver;
	globalThis.IntersectionObserver = QuietObserver as unknown as typeof IntersectionObserver;
	// A card sits at its row in the grid: one card per 300px row.
	savedOffsetTop = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetTop');
	savedOffsetHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetHeight');
	Object.defineProperty(HTMLElement.prototype, 'offsetTop', {
		configurable: true,
		get(this: HTMLElement) {
			if (!this.dataset.cardId || !this.parentElement) return 0;
			return [...this.parentElement.children].indexOf(this) * ROW;
		}
	});
	Object.defineProperty(HTMLElement.prototype, 'offsetHeight', {
		configurable: true,
		get(this: HTMLElement) {
			return this.dataset.cardId ? ROW : 0;
		}
	});
	window.scrollTo(0, 0);
	setUrl('/search');
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	globalThis.IntersectionObserver = savedObserver as typeof IntersectionObserver;
	if (savedOffsetTop) Object.defineProperty(HTMLElement.prototype, 'offsetTop', savedOffsetTop);
	if (savedOffsetHeight)
		Object.defineProperty(HTMLElement.prototype, 'offsetHeight', savedOffsetHeight);
	window.scrollTo(0, 0);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

async function settle(steps = 30) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

function buttonNamed(name: string): HTMLButtonElement | null {
	return [...target.querySelectorAll('button')].find((b) => b.textContent?.trim() === name) ?? null;
}

/** The titles in the grid, in order. */
const cards = () =>
	[...target.querySelectorAll('[data-card-id]')].map(
		(el) => el.querySelector('.card-title')?.textContent?.trim() ?? ''
	);
const query = () => target.querySelector('.eyebrow-value')?.textContent?.trim() ?? '';
const orderChosen = (label: string) => buttonNamed(label)?.getAttribute('aria-checked') === 'true';
const typeOn = (label: string) => buttonNamed(label)?.getAttribute('aria-pressed') === 'true';

/** Report a navigation to the hooks the mounted page registered. */
function navigated(type: 'popstate' | 'enter' | 'link') {
	for (const [callback] of vi.mocked(afterNavigate).mock.calls) {
		(callback as (nav: unknown) => void)({ type, from: null, to: { url: page.url } });
	}
}

/** A back or forward press is starting, to `q` on the search page. */
function pressingBackTo(q: string) {
	const to = new URL(page.url.href);
	to.searchParams.set('q', q);
	for (const [callback] of vi.mocked(beforeNavigate).mock.calls) {
		(callback as (nav: unknown) => void)({ type: 'popstate', to: { url: to }, cancel() {} });
	}
}

function mountSearch() {
	vi.mocked(afterNavigate).mockClear();
	vi.mocked(beforeNavigate).mockClear();
	app = mount(SearchPage, { target });
}

type SnapshotCarrier = { snapshot: { capture: () => unknown; restore: (v: unknown) => void } };

/** Leave the page as SvelteKit does: capture, keep it as JSON, unmount. */
function leave(): unknown {
	const kept = JSON.parse(JSON.stringify((app as unknown as SnapshotCarrier).snapshot.capture()));
	unmount(app!);
	app = null;
	// The show page opens at its top.
	window.scrollTo(0, 0);
	return kept;
}

/** Come back to the entry as SvelteKit does: mount, restore, report. */
function comeBack(kept: unknown, type: 'popstate' | 'enter' = 'popstate') {
	mountSearch();
	(app as unknown as SnapshotCarrier).snapshot.restore(kept);
	navigated(type);
}

/**
 * The reader searches "frieren", orders by year, keeps TV series, loads
 * a second page and scrolls so the eleventh TV series — show 21, the
 * first of page two — has its top 100px above the top of the screen.
 */
async function browse() {
	setQuery('frieren');
	mountSearch();
	navigated('link');
	await until(() => cards().includes('Show 1'), 'the first page');
	buttonNamed(m.search_sort_year())!.click();
	await until(() => asked.includes('frieren@0/year'), 'the search in year order');
	await until(() => cards().includes('Show 1') && !cards().includes('Show 21'), 'its first page');
	buttonNamed(subtypeLabel('TV'))!.click();
	await until(() => buttonNamed(m.search_paging_load_more()) !== null, 'Load more');
	buttonNamed(m.search_paging_load_more())!.click();
	await until(() => cards().includes('Show 21'), 'the second page');
	await settle();
	window.scrollTo(0, 10 * ROW + 100);
	return cards();
}

describe('back to the search page', () => {
	it('brings back the query, order, type filters, results and scroll without asking again', async () => {
		const before = await browse();
		const askedBefore = [...asked];
		const kept = leave();

		comeBack(kept);
		await until(() => cards().length === before.length, 'the results');
		await settle();

		expect(query()).toBe('frieren');
		expect(orderChosen(m.search_sort_year())).toBe(true);
		expect(typeOn(subtypeLabel('TV'))).toBe(true);
		expect(cards()).toEqual(before);
		expect(asked, 'no page was asked for again').toEqual(askedBefore);
		expect(window.scrollY).toBe(10 * ROW + 100);
	});

	it('goes on loading from the page after the ones it brought back', async () => {
		await browse();
		const kept = leave();
		comeBack(kept);
		await until(() => cards().includes('Show 39'), 'the results');
		asked.length = 0;

		buttonNamed(m.search_paging_load_more())!.click();
		await until(() => cards().includes('Show 41'), 'the third page');
		expect(asked).toEqual(['frieren@40/year']);
	});

	it('asks again, in the order and filters left, once the results are an hour old', async () => {
		const before = await browse();
		const kept = leave() as { results: { fetchedAt: number } };
		kept.results.fetchedAt -= 2 * 60 * 60 * 1000;
		asked.length = 0;

		comeBack(kept);
		await until(() => cards().includes('Show 21'), 'the search up to the card');
		await settle();

		expect(asked).toEqual(['frieren@0/year', 'frieren@20/year']);
		expect(orderChosen(m.search_sort_year())).toBe(true);
		expect(typeOn(subtypeLabel('TV'))).toBe(true);
		expect(cards()).toEqual(before);
		expect(window.scrollY, 'the same card is back at the top').toBe(10 * ROW + 100);
	});

	it('asks again when the audio setting changed while the reader was away', async () => {
		await browse();
		const kept = leave();
		mode = 'dub';
		asked.length = 0;

		comeBack(kept);
		await until(() => cards().includes('Show 21'), 'the search up to the card');
		expect(asked).toEqual(['frieren@0/year', 'frieren@20/year']);
	});

	it('asks again on a reload, keeping the order and filters', async () => {
		await browse();
		const kept = leave();
		asked.length = 0;

		comeBack(kept, 'enter');
		await until(() => cards().includes('Show 21'), 'the search up to the card');
		await settle();
		expect(asked).toEqual(['frieren@0/year', 'frieren@20/year']);
		expect(typeOn(subtypeLabel('TV'))).toBe(true);
		expect(window.scrollY).toBe(10 * ROW + 100);
	});

	it('follows the address when the entry kept nothing', async () => {
		setQuery('himmel');
		mountSearch();
		navigated('popstate');
		await until(() => cards().includes('Show 1'), 'the search');
		expect(asked).toEqual(['himmel@0/relevance']);
	});

	it('does not bring back an earlier search on a page reached afresh', async () => {
		await browse();
		leave();
		asked.length = 0;

		// The topbar submits the same query again: a new entry, no restore.
		setQuery('frieren');
		mountSearch();
		navigated('link');
		await until(() => cards().includes('Show 1'), 'the search');
		expect(asked).toEqual(['frieren@0/relevance']);
		expect(orderChosen(m.search_sort_relevance())).toBe(true);
		expect(typeOn(subtypeLabel('TV'))).toBe(false);
		expect(window.scrollY).toBe(0);
	});

	it('brings back the earlier query after a new one on the same page', async () => {
		const before = await browse();
		const kept = (app as unknown as SnapshotCarrier).snapshot.capture();
		const askedBefore = [...asked];

		// A new query from the topbar while on the search page.
		setQuery('himmel');
		await until(() => asked.includes('himmel@0/year'), 'the new search');
		await until(() => query() === 'himmel', 'the new query');
		window.scrollTo(0, 0);

		pressingBackTo('frieren');
		setQuery('frieren');
		(app as unknown as SnapshotCarrier).snapshot.restore(JSON.parse(JSON.stringify(kept)));
		navigated('popstate');
		await until(() => query() === 'frieren', 'the earlier query');
		await until(() => cards().length === before.length, 'its results');
		await settle();

		expect(cards()).toEqual(before);
		expect(asked).toEqual([...askedBefore, 'himmel@0/year']);
		expect(window.scrollY).toBe(10 * ROW + 100);
	});

	it('opens empty, with the order and filters back to their defaults, from the bare search link', async () => {
		await browse();
		asked.length = 0;

		// The rail's Search link, while the search is on screen.
		setUrl('/search');
		await until(() => cards().length === 0, 'the results to go');
		expect(query()).toBe(m.search_eyebrow_value_empty());
		expect(target.textContent).toContain(m.search_subtitle_empty());

		setQuery('himmel');
		await until(() => cards().includes('Show 1'), 'the next search');
		expect(asked).toEqual(['himmel@0/relevance']);
		expect(orderChosen(m.search_sort_relevance())).toBe(true);
		expect(typeOn(subtypeLabel('TV'))).toBe(false);
	});

	it('opens empty on a back press onto the bare search page', async () => {
		await browse();
		const kept = (app as unknown as SnapshotCarrier).snapshot.capture();
		// The entry behind is the bare search page, which kept an empty search.
		unmount(app!);
		app = null;
		setUrl('/search');
		mountSearch();
		const empty = (app as unknown as SnapshotCarrier).snapshot.capture();
		unmount(app!);
		app = null;
		setQuery('frieren');
		mountSearch();
		(app as unknown as SnapshotCarrier).snapshot.restore(JSON.parse(JSON.stringify(kept)));
		navigated('popstate');
		await until(() => cards().includes('Show 21'), 'the restored search');

		pressingBackTo('');
		setUrl('/search');
		(app as unknown as SnapshotCarrier).snapshot.restore(JSON.parse(JSON.stringify(empty)));
		navigated('popstate');
		await until(() => cards().length === 0, 'the results to go');
		expect(query()).toBe(m.search_eyebrow_value_empty());
	});

	it('asks again on the way back from a search left open for over an hour', async () => {
		const before = await browse();
		// Two hours pass on the search page before the reader opens a show.
		const realNow = Date.now.bind(Date);
		const later = vi.spyOn(Date, 'now').mockImplementation(() => realNow() + 2 * 60 * 60 * 1000);
		try {
			const kept = leave();
			asked.length = 0;
			comeBack(kept);
			await until(() => cards().includes('Show 21'), 'the search up to the card');
			await settle();
			expect(asked).toEqual(['frieren@0/year', 'frieren@20/year']);
			expect(cards()).toEqual(before);
		} finally {
			later.mockRestore();
		}
	});
});
