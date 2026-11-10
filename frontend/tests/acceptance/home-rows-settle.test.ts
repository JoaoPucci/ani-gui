// Acceptance: Home's rows load on their own, each when its data lands,
// without a row appearing and another then pushing it down or pulling
// it up.
//
// Below the hero, Home stacks Continue watching (the history read),
// Watch later (the saved sign-ins, then the tracker list), Trending now
// and Top rated (two catalogue requests). None waits for another. Top
// rated is cached longer than Trending, so it usually lands first, and
// was drawn at once; when Trending then failed or came back with
// nothing for the row, Trending's skeleton went and Top rated jumped up
// into its place. A history read or a sign-in answering after the
// catalogue rows put a whole row above them. These scenarios hold each
// request in turn and look at the page in between.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse, type HttpHandler } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';

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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { markRefreshed } from '../../src/lib/account/watch-later-refresh';
import { listChanges } from '../../src/lib/account/list-changes';
import { m } from '../../src/lib/paraglide/messages';

// Eight trending shows: the hero takes the first three, the row lists
// the rest, from "Airing 4" on.
const TRENDING = Array.from({ length: 8 }, (_, i) => kitsuRef(`t${i + 1}`, `Airing ${i + 1}`, 12));
const TOP_RATED = [kitsuRef('r1', 'Classic One', 12), kitsuRef('r2', 'Classic Two', 12)];
const PLANNED = [kitsuRef('w1', 'Planned One', 12)];
const HISTORY = [{ id: 'h1', ep_no: '3', title: 'Watched Show' }];

const SIGN_IN = {
	access_token: 'test-bearer',
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id: 'user-1',
	username: 'tester',
	avatar_url: null
};

/** A request held until the scenario lets it answer. */
function held() {
	let release: () => void = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { gate, release };
}

/** Ids the availability filter has been asked about: a row whose ids
 *  are here has been handed back to the page. */
const filtered = new Set<string>();
/** The sub/dub mode each availability read asked in. */
const modes: string[] = [];
/** How many times the Watch later list was read. */
let listReads = 0;

interface Scenario {
	trending?: Promise<void>;
	topRated?: Promise<void>;
	history?: Promise<void>;
	list?: Promise<void>;
	settings?: Promise<void>;
	trendingFails?: boolean;
	trendingShows?: typeof TRENDING;
	planned?: typeof PLANNED;
	historyRows?: typeof HISTORY;
	mode?: 'sub' | 'dub';
}

function handlers(s: Scenario): HttpHandler[] {
	return homeHandlers({ history: s.historyRows ?? [], mode: s.mode }, [
		http.get(`${API_BASE}/api/settings`, async () => {
			await s.settings;
			return HttpResponse.json(appConfig(s.mode));
		}),
		http.get(`${API_BASE}/api/history`, async () => {
			await s.history;
			return HttpResponse.json(s.historyRows ?? []);
		}),
		http.get(`${API_BASE}/api/kitsu/trending-anilist`, async () => {
			await s.trending;
			return s.trendingFails
				? HttpResponse.json({ error: 'error.network' }, { status: 503 })
				: HttpResponse.json(s.trendingShows ?? TRENDING);
		}),
		http.get(`${API_BASE}/api/kitsu/top-rated`, async () => {
			await s.topRated;
			return HttpResponse.json(TOP_RATED);
		}),
		http.get(`${API_BASE}/api/account/list/anilist/cached`, async () => {
			listReads += 1;
			await s.list;
			return HttpResponse.json(
				(s.planned ?? PLANNED).map((p, i) => ({
					provider: 'anilist',
					media_id: 100 + i,
					mal_id: 200 + i,
					status: 'planning',
					progress_episodes: 0,
					score_0_to_100: null,
					updated_at_epoch_s: 1_700_000_000 - i,
					title: p.canonical_title
				}))
			);
		}),
		http.post(`${API_BASE}/api/kitsu/by-mal-ids`, () => HttpResponse.json(s.planned ?? PLANNED)),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
			for (const id of body.kitsu_ids) filtered.add(id);
			modes.push(body.mode);
			return HttpResponse.json({
				cached: Object.fromEntries(body.kitsu_ids.map((id) => [id, true])),
				playable_episode_counts: {}
			});
		})
	]);
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
const gates: (() => void)[] = [];
/** A held request, released at teardown at the latest: one still
 *  unanswered when the page unmounts fails the scenario. */
function hold() {
	const h = held();
	gates.push(h.release);
	return h;
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	filtered.clear();
	modes.length = 0;
	listReads = 0;
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	for (const release of gates.splice(0)) release();
	await settle();
	if (app) unmount(app);
	app = null;
	target.remove();
	accountStore.setDisconnected('anilist');
	accountStore.readsSettled = true;
	accountStore.keyringWaiting = false;
	localStorage.clear();
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
const settle = () => new Promise((r) => setTimeout(r, 80));

const screen = () => target.textContent ?? '';

/** The strip drawn for the row named `eyebrow`, if there is one: a
 *  strip's scroller carries its row's name. */
const row = (eyebrow: string) =>
	target.querySelector(`[aria-label="${eyebrow}"]`)?.closest('section.strip') ?? null;
const isSkeleton = (strip: Element | null) => strip !== null && !!strip.querySelector('.skel-card');
/** The rows drawn, top to bottom, by name, a skeleton marked so. */
const drawn = () =>
	[...target.querySelectorAll('section.strip')].map((s) => {
		const name = s.querySelector('.eyebrow-key')?.textContent?.trim() ?? '?';
		return isSkeleton(s) ? `${name} (loading)` : name;
	});

const CONTINUE = m.home_strip_continue_eyebrow();
const WATCH_LATER = m.account_watch_later_title();
const TRENDING_ROW = m.home_strip_trending_eyebrow();
const TOP_RATED_ROW = m.home_strip_top_rated_eyebrow();

describe('Home: the rows below the hero', () => {
	it('holds Trending and Top rated as skeletons in their final order, the height of the cards they wait for', async () => {
		server.use(...handlers({ trending: hold().gate, topRated: hold().gate }));

		app = mount(HomePage, { target });
		await until(() => drawn().length === 2, 'both catalogue rows as skeletons');

		expect(drawn()).toEqual([`${TRENDING_ROW} (loading)`, `${TOP_RATED_ROW} (loading)`]);
		// A placeholder card keeps room for a title wrapped to two lines,
		// as a poster card's title often is, so a row does not grow as it
		// fills.
		const cards = target.querySelectorAll('.skel-card');
		expect(cards.length).toBeGreaterThan(0);
		for (const card of cards) expect(card.querySelectorAll('.skel-title').length).toBe(2);
	});

	it('keeps Top rated a skeleton while Trending, above it, is still loading', async () => {
		const trending = hold();
		server.use(...handlers({ trending: trending.gate }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1'), 'Top rated to come back from the filter');
		await settle();

		// Drawn now, it would move when Trending lands above it.
		expect(screen()).not.toContain('Classic One');
		expect(drawn()).toEqual([`${TRENDING_ROW} (loading)`, `${TOP_RATED_ROW} (loading)`]);

		trending.release();
		await until(() => screen().includes('Classic One'), 'Top rated once Trending lands');
		expect(drawn()).toEqual([TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('shows Top rated in Trending’s place only once Trending has failed', async () => {
		const trending = hold();
		server.use(...handlers({ trending: trending.gate, trendingFails: true }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1'), 'Top rated to come back from the filter');
		await settle();
		expect(screen()).not.toContain('Classic One');

		trending.release();
		await until(() => screen().includes('Classic One'), 'Top rated once Trending fails');
		expect(drawn()).toEqual([TOP_RATED_ROW]);
		// The hero says the catalogue did not answer.
		expect(screen()).toContain(m.home_hero_error_title());
	});

	it('shows Top rated in Trending’s place only once Trending turns out to have nothing for the row', async () => {
		// Three trending shows all go to the hero; the row has none left.
		const trending = hold();
		server.use(...handlers({ trending: trending.gate, trendingShows: TRENDING.slice(0, 3) }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1'), 'Top rated to come back from the filter');
		await settle();
		expect(screen()).not.toContain('Classic One');

		trending.release();
		await until(() => screen().includes('Classic One'), 'Top rated once Trending lands');
		expect(drawn()).toEqual([TOP_RATED_ROW]);
	});

	it('draws no row while the history read is out, then Continue watching above the rest', async () => {
		// Continue watching has no placeholder; a row drawn below it
		// before the read answers would be pushed down when it does.
		const history = hold();
		server.use(...handlers({ history: history.gate, historyRows: HISTORY }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1') && filtered.has('t4'), 'both catalogue rows');
		await settle();
		expect(drawn()).toEqual([]);

		history.release();
		await until(() => row(CONTINUE) !== null, 'Continue watching');
		await settle();
		expect(drawn()).toEqual([CONTINUE, TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('draws no row below Watch later until the saved sign-ins have been read', async () => {
		// The store reads as signed out until the keyring answers. Rows
		// drawn meanwhile would be pushed down by the Watch later row a
		// sign-in puts above them.
		accountStore.readsSettled = false;
		server.use(...handlers({}));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1') && filtered.has('t4'), 'both catalogue rows');
		await settle();
		expect(drawn()).toEqual([]);

		// The keyring answers: signed in.
		accountStore.setConnected('anilist', SIGN_IN);
		accountStore.readsSettled = true;
		await until(() => screen().includes('Planned One'), 'Watch later');
		await settle();
		expect(drawn()).toEqual([WATCH_LATER, TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('draws the rows once the keyring is slow enough to say so', async () => {
		// A locked keyring can wait on its unlock prompt for good. Once the
		// app has said it is waiting, the rows below are not held for it.
		accountStore.readsSettled = false;
		server.use(...handlers({}));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1') && filtered.has('t4'), 'both catalogue rows');
		await settle();
		expect(drawn()).toEqual([]);

		accountStore.keyringWaiting = true;
		await until(() => screen().includes('Classic One'), 'the catalogue rows');
		// The notice going down again as the reads answer does not take
		// the rows away.
		accountStore.keyringWaiting = false;
		await settle();
		expect(drawn()).toEqual([TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('loads Watch later as a skeleton, not an empty list, when the keyring answers after its notice', async () => {
		// The rows were drawn once the notice went up, and Watch later
		// was read with no sign-in. A sign-in landing now is a new list,
		// not the empty one read without it.
		accountStore.readsSettled = false;
		const list = hold();
		server.use(...handlers({ list: list.gate }));

		app = mount(HomePage, { target });
		accountStore.keyringWaiting = true;
		await until(() => screen().includes('Classic One'), 'the catalogue rows');

		accountStore.setConnected('anilist', SIGN_IN);
		accountStore.readsSettled = true;
		accountStore.keyringWaiting = false;
		await until(() => row(WATCH_LATER) !== null, 'Watch later');
		await settle();
		expect(screen()).not.toContain(m.account_watch_later_empty());
		// The rows already shown stay shown: they are not taken back to
		// skeletons behind the row that came in above them.
		expect(drawn()).toEqual([`${WATCH_LATER} (loading)`, TRENDING_ROW, TOP_RATED_ROW]);

		list.release();
		await until(() => screen().includes('Planned One'), 'Watch later');
		expect(drawn()).toEqual([WATCH_LATER, TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('holds a Watch later skeleton while a signed-in list loads, and the rows below with it', async () => {
		accountStore.setConnected('anilist', SIGN_IN);
		const list = hold();
		server.use(...handlers({ list: list.gate }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1') && filtered.has('t4'), 'both catalogue rows');
		await settle();
		// Not "Nothing planned yet": the list has not said so.
		expect(screen()).not.toContain(m.account_watch_later_empty());
		expect(drawn()).toEqual([
			`${WATCH_LATER} (loading)`,
			`${TRENDING_ROW} (loading)`,
			`${TOP_RATED_ROW} (loading)`
		]);
		expect(screen()).toContain(m.account_watch_later_caption());

		list.release();
		await until(() => screen().includes('Planned One'), 'Watch later');
		await settle();
		expect(drawn()).toEqual([WATCH_LATER, TRENDING_ROW, TOP_RATED_ROW]);
	});

	it('keeps the Watch later row of an expired sign-in whose list is empty', async () => {
		// The list of an expired sign-in is still read. Whether the row is
		// there must not wait on the sign-in's renewal, which would bring
		// the empty row in above rows already drawn.
		accountStore.setExpired('anilist', { ...SIGN_IN, expires_at_epoch_s: 1 });
		server.use(...handlers({ planned: [] }));

		app = mount(HomePage, { target });
		await until(() => screen().includes('Classic One'), 'the catalogue rows');
		await settle();
		expect(drawn()).toEqual([WATCH_LATER, TRENDING_ROW, TOP_RATED_ROW]);
		expect(screen()).toContain(m.account_watch_later_empty());
	});

	it("filters every row in the viewer's mode, reading the Watch later list once", async () => {
		accountStore.setConnected('anilist', SIGN_IN);
		// Pulled from the tracker a moment ago: the row's freshness check
		// does not pull it again, which reads the list a second time on
		// purpose.
		markRefreshed('anilist', Date.now());
		server.use(...handlers({ mode: 'dub' }));

		app = mount(HomePage, { target });
		await until(
			() => screen().includes('Planned One') && screen().includes('Classic One'),
			'every row'
		);
		await settle();
		expect(modes.length).toBeGreaterThan(0);
		expect(new Set(modes)).toEqual(new Set(['dub']));
		expect(listReads).toBe(1);
	});

	it("leaves a list change before the settings answer to the first load, in the viewer's mode", async () => {
		// A tracker write that settles while Home is still starting asks
		// the row to read its list again. Read then, before the settings,
		// it would be filtered for sub and read twice.
		accountStore.setConnected('anilist', SIGN_IN);
		markRefreshed('anilist', Date.now());
		const settings = hold();
		server.use(...handlers({ mode: 'dub', settings: settings.gate }));

		app = mount(HomePage, { target });
		await settle();
		listChanges.announce({
			kitsuId: 'w1',
			providers: ['anilist'],
			landed: ['anilist'],
			source: 'editor'
		});
		await settle();
		settings.release();
		await until(() => screen().includes('Planned One'), 'Watch later');
		await settle();
		expect(new Set(modes)).toEqual(new Set(['dub']));
		expect(listReads).toBe(1);
	});

	it('draws the rows without Continue watching once a hung history read runs past its wait', async () => {
		// The history read does not answer. Past the wait the rows below
		// the hero are drawn as if there were no history yet.
		const history = hold();
		server.use(...handlers({ history: history.gate, historyRows: HISTORY }));

		app = mount(HomePage, { target });
		await until(() => filtered.has('r1') && filtered.has('t4'), 'both catalogue rows');
		await settle();
		expect(drawn()).toEqual([]);

		await until(() => screen().includes('Classic One'), 'the rows past the wait', 6000);
		expect(drawn()).toEqual([TRENDING_ROW, TOP_RATED_ROW]);
		expect(row(CONTINUE)).toBeNull();
	});

	it('brings Continue watching in above the rows when the history answers after the wait, keeping them shown', async () => {
		const history = hold();
		server.use(...handlers({ history: history.gate, historyRows: HISTORY }));

		app = mount(HomePage, { target });
		await until(() => screen().includes('Classic One'), 'the rows past the wait', 6000);

		history.release();
		await until(() => row(CONTINUE) !== null, 'Continue watching');
		await settle();
		expect(drawn()).toEqual([CONTINUE, TRENDING_ROW, TOP_RATED_ROW]);
	});
});
