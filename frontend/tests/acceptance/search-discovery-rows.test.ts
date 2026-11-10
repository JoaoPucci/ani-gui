// Acceptance: the search page opened with no query (the home page's
// "browse the catalogue" path) draws its two discovery rows, Trending
// now above Top rated, without one row appearing and then another
// pushing it down.
//
// The two rows are separate requests. Trending is the home page's
// AniList list (/api/kitsu/trending-anilist; the Kitsu-only
// /api/kitsu/trending is not asked, and an unhandled request fails the
// scenario). Top rated is cached for six hours and Trending for one,
// so on most visits Top rated answers first. Drawn as it landed, it
// took the top of the page, and when Trending arrived a moment later
// it was inserted above and the row the reader was looking at slid
// down. These scenarios hold each request in turn and look at the page
// in between.

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

const TRENDING = [ref('t1', 'Airing One'), ref('t2', 'Airing Two')];
const TOP_RATED = [ref('r1', 'Classic One'), ref('r2', 'Classic Two')];

const CONFIG = {
	locale: 'en',
	mode: 'sub',
	download_quality: 'best',
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

/** A request held until the scenario lets it answer. */
function held() {
	let release: () => void = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { gate, release };
}

/** Ids the availability filter has answered for: a row whose ids are
 *  here has been handed back to the page. */
const filtered = new Set<string>();

function handlers(opts: {
	trending?: Promise<void>;
	topRated?: Promise<void>;
	trendingFails?: boolean;
	topRatedFails?: boolean;
}) {
	return [
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(CONFIG)),
		http.get(`${API_BASE}/api/kitsu/trending-anilist`, async () => {
			await opts.trending;
			return opts.trendingFails
				? HttpResponse.json({ error: 'error.network' }, { status: 503 })
				: HttpResponse.json(TRENDING);
		}),
		http.get(`${API_BASE}/api/kitsu/top-rated`, async () => {
			await opts.topRated;
			return opts.topRatedFails
				? HttpResponse.json({ error: 'error.network' }, { status: 503 })
				: HttpResponse.json(TOP_RATED);
		}),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const { kitsu_ids } = (await request.json()) as { kitsu_ids: string[] };
			for (const id of kitsu_ids) filtered.add(id);
			return HttpResponse.json({
				cached: Object.fromEntries(kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		})
	];
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	filtered.clear();
	setQuery('');
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
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

/** Give the page a few turns to draw whatever it is going to draw. */
const settle = () => new Promise((r) => setTimeout(r, 60));

const screen = () => target.textContent ?? '';
const skeletons = () => target.querySelectorAll('.skel-card').length;

/** The last element in document order whose text includes `text`:
 *  the innermost one, as long as `text` sits in a single element. */
function at(text: string): Element {
	const all = [...target.querySelectorAll('*')].filter((e) => e.textContent?.includes(text));
	const el = all.at(-1);
	if (!el) throw new Error(`"${text}" is not on the page\n--- DOM ---\n${screen()}`);
	return el;
}
/** The strip drawn for the row named `eyebrow`, if there is one: a
 *  strip's scroller carries its row's name. The eyebrow text alone is
 *  no evidence, because the page's caption above the rows repeats it. */
const row = (eyebrow: string) => target.querySelector(`[aria-label="${eyebrow}"]`);
/** The page caption under the Catalogue eyebrow. It stays in place
 *  whatever the rows do, keeping its line, and is hidden (from sight and
 *  from screen readers) when the rows it names are not all there. */
function caption(): Element {
	const el = [...target.querySelectorAll('p')].find((p) =>
		p.textContent?.includes(m.search_empty_caption())
	);
	if (!el) throw new Error(`the page caption is not on the page\n--- DOM ---\n${screen()}`);
	return el;
}
/** Shown to sight and to screen readers: the caption-hidden class
 *  carries the visibility rule (happy-dom applies no component CSS, so
 *  the class is the observable half) and aria-hidden the other. */
const captionShown = () => {
	const el = caption();
	const hiddenToSight = el.classList.contains('caption-hidden');
	const hiddenToReaders = el.getAttribute('aria-hidden') === 'true';
	expect(hiddenToSight).toBe(hiddenToReaders);
	return !hiddenToSight;
};
const above = (a: string, b: string) =>
	(at(a).compareDocumentPosition(at(b)) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0;

describe('/search with no query: the discovery rows', () => {
	it('holds both rows as skeletons while they load', async () => {
		const trending = held();
		const topRated = held();
		server.use(...handlers({ trending: trending.gate, topRated: topRated.gate }));

		app = mount(SearchPage, { target });
		await until(() => skeletons() > 0, 'skeleton rows while both rows load');

		const trendingRow = row(m.search_discovery_trending_eyebrow());
		const topRatedRow = row(m.search_discovery_top_rated_eyebrow());
		expect(trendingRow).not.toBeNull();
		expect(topRatedRow).not.toBeNull();
		// In their final order: Trending above Top rated.
		expect(
			trendingRow!.compareDocumentPosition(topRatedRow!) & Node.DOCUMENT_POSITION_FOLLOWING
		).not.toBe(0);
		expect(screen()).toContain(m.search_discovery_trending_caption_loading());
		// A placeholder card keeps room for a title wrapped to two lines,
		// as most rows of twenty have, so a row does not grow as it fills.
		for (const card of target.querySelectorAll('.skel-card')) {
			expect(card.querySelectorAll('.skel-title').length).toBe(2);
		}

		trending.release();
		topRated.release();
		await until(
			() => screen().includes('Airing One') && screen().includes('Classic One'),
			'both rows once both land'
		);
		expect(skeletons()).toBe(0);
	});

	it('does not show Top rated alone while Trending, above it, is still loading', async () => {
		const trending = held();
		server.use(...handlers({ trending: trending.gate }));

		app = mount(SearchPage, { target });
		await until(() => filtered.has('r1'), 'Top rated to come back from the filter');
		await settle();

		// Drawn now, it would be pushed down when Trending arrives.
		expect(screen()).not.toContain('Classic One');
		expect(skeletons()).toBeGreaterThan(0);

		trending.release();
		await until(() => screen().includes('Airing One'), 'Trending to land');

		expect(screen()).toContain('Classic One');
		expect(above('Airing One', 'Classic One')).toBe(true);
		expect(skeletons()).toBe(0);
	});

	it('shows Trending at once and fills Top rated in below it when that lands', async () => {
		const topRated = held();
		server.use(...handlers({ topRated: topRated.gate }));

		app = mount(SearchPage, { target });
		await until(() => screen().includes('Airing One'), 'Trending to land');

		expect(screen()).not.toContain('Classic One');
		// Top rated keeps its place below as a skeleton.
		expect(skeletons()).toBeGreaterThan(0);
		const topRatedRow = row(m.search_discovery_top_rated_eyebrow());
		expect(topRatedRow).not.toBeNull();
		expect(
			at('Airing One').compareDocumentPosition(topRatedRow!) & Node.DOCUMENT_POSITION_FOLLOWING
		).not.toBe(0);

		topRated.release();
		await until(() => screen().includes('Classic One'), 'Top rated to land');
		expect(above('Airing One', 'Classic One')).toBe(true);
		expect(skeletons()).toBe(0);
	});

	it('shows Top rated on its own when Trending fails', async () => {
		server.use(...handlers({ trendingFails: true }));

		app = mount(SearchPage, { target });
		await until(() => screen().includes('Classic One'), 'Top rated to show');
		await settle();

		expect(skeletons()).toBe(0);
		expect(row(m.search_discovery_trending_eyebrow())).toBeNull();
		// The page caption names both rows, and one of them is not there.
		expect(captionShown()).toBe(false);
		expect(screen()).not.toContain(m.search_discovery_nothing_headline());
	});

	it('keeps Trending where it is when Top rated fails after Trending is shown', async () => {
		const topRated = held();
		server.use(...handlers({ topRated: topRated.gate, topRatedFails: true }));

		app = mount(SearchPage, { target });
		await until(() => screen().includes('Airing One'), 'Trending to land');
		expect(captionShown()).toBe(true);
		const shownCaption = caption();
		const trendingRow = row(m.search_discovery_trending_eyebrow());

		topRated.release();
		await until(() => skeletons() === 0, 'Top rated to be dropped');

		expect(row(m.search_discovery_top_rated_eyebrow())).toBeNull();
		// Nothing above Trending changed: the caption is the same element,
		// still holding its line, only hidden; Trending is the same strip.
		expect(caption()).toBe(shownCaption);
		expect(captionShown()).toBe(false);
		expect(row(m.search_discovery_trending_eyebrow())).toBe(trendingRow);
	});

	it('says there is nothing to browse when both rows fail, and tries again on request', async () => {
		server.use(...handlers({ trendingFails: true, topRatedFails: true }));

		app = mount(SearchPage, { target });
		await until(
			() => screen().includes(m.search_discovery_nothing_headline()),
			'the nothing-to-browse notice'
		);
		expect(screen()).toContain(m.search_discovery_nothing_detail());
		expect(skeletons()).toBe(0);
		// The caption above names rows that are not there.
		expect(captionShown()).toBe(false);

		// The catalogue answers again.
		server.use(...handlers({}));
		const retry = [...target.querySelectorAll('button')].find(
			(b) => b.textContent?.trim() === m.search_discovery_retry()
		);
		expect(retry).toBeDefined();
		retry!.focus();
		retry!.click();
		// The button leaves as the rows start loading again. Focus that
		// sat on it stays on the page instead of falling to its body.
		await until(() => !target.contains(retry!), 'Try again to leave');
		await settle();
		expect(document.activeElement).not.toBe(document.body);
		expect(target.contains(document.activeElement)).toBe(true);

		await until(
			() => screen().includes('Airing One') && screen().includes('Classic One'),
			'both rows after trying again'
		);
		expect(screen()).not.toContain(m.search_discovery_nothing_headline());
		expect(captionShown()).toBe(true);
		expect(above('Airing One', 'Classic One')).toBe(true);
	});

	it('reads the settings again on Try again when they could not be read before', async () => {
		// The settings come from the same local backend as both rows, so
		// the usual way to reach the notice also loses the sub/dub mode.
		const modes: string[] = [];
		const recording = (settingsFail: boolean, rowsFail: boolean) => [
			http.get(`${API_BASE}/api/settings`, () =>
				settingsFail
					? HttpResponse.json({ error: 'error.network' }, { status: 503 })
					: HttpResponse.json({ ...CONFIG, mode: 'dub' })
			),
			...handlers({ trendingFails: rowsFail, topRatedFails: rowsFail }).slice(1, 3),
			http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
				const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
				modes.push(body.mode);
				return HttpResponse.json({
					cached: Object.fromEntries(body.kitsu_ids.map((id) => [id, true])),
					playable_episode_counts: {}
				});
			})
		];
		server.use(...recording(true, true));

		app = mount(SearchPage, { target });
		await until(
			() => screen().includes(m.search_discovery_nothing_headline()),
			'the nothing-to-browse notice'
		);

		server.use(...recording(false, false));
		[...target.querySelectorAll('button')]
			.find((b) => b.textContent?.trim() === m.search_discovery_retry())!
			.click();

		await until(
			() => screen().includes('Airing One') && screen().includes('Classic One'),
			'both rows after trying again'
		);
		expect(modes).toEqual(['dub', 'dub']);
	});
});
