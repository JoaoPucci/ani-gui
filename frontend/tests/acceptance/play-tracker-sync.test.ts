// Acceptance: an episode watched on the play page reaches every
// connected tracker's write route with the progress and status the
// write-back rule gives it.
//
// The page records a play once the switch to it lands: it stamps
// Continue Watching and posts the episode to each connected tracker's
// progress route. Whether that post carries `completed` is decided
// on the page, from the Kitsu show it loaded: only Kitsu's own last
// episode of a finished show sends it. Everything that depends on the
// tracker's own row — its episode total, a rewatch, progress already
// past the episode — is decided by the backend behind that route. A
// show whose episode map leaves it as Kitsu numbers it is covered
// against MAL and AniList (a shorter entry against MAL only) in
// `backend/src/commands/account_tracker_total_test.rs`. A show the map
// files under other entries or numbers is written by
// `backend/src/commands/account_mapped.rs`, which decides each entry's
// completion from the map and does not read the page's `completed`;
// that is covered in `backend/src/commands/account_mapped_test.rs`,
// and the requests each tracker receives for it in
// `account_mapped_wire_test.rs`. These scenarios pin the page's half:
// what each tracker's route receives.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
// A landed switch navigates to the episode it played; following it
// keeps the page's own episode where the real router would put it, so
// a click on the episode already playing does nothing here either.
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async (href: string) => {
		const next = new URL(href, 'http://127.0.0.1:31337');
		setUrl(next.pathname, Object.fromEntries(next.searchParams));
	}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { Provider } from '../../src/lib/account/types';

const KITSU_ID = '42';
const TITLE = 'Finished Show';
const TRACKERS: Provider[] = ['mal', 'anilist'];

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	landed = false;
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
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

interface TrackerWrite {
	provider: string;
	body: Record<string, unknown>;
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let writes: TrackerWrite[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	writes = [];
	setParams({ id: KITSU_ID });
	// The write-back fans out to every connected tracker. `hydrate()`
	// reads the Electron keychain from the layout, which a page-level
	// mount never reaches — so seed the store the way hydrate would.
	for (const provider of TRACKERS) {
		accountStore.setConnected(provider, {
			access_token: `${provider}-bearer`,
			refresh_token: null,
			expires_at_epoch_s: 4_000_000_000,
			user_id: 'user-1',
			username: 'tester',
			avatar_url: null
		});
	}
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	for (const provider of TRACKERS) accountStore.setDisconnected(provider);
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const card = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;

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

interface Show {
	/** Kitsu's episode count for the show. */
	kitsuCount: number;
	/** Kitsu's status: `finished` is the only one that completes. */
	status: string;
	/** How many episodes the provider lists for the audio in use. */
	providerCount: number;
	mode?: 'sub' | 'dub';
}

function serve(show: Show) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(show.mode ?? 'sub'))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, show.kitsuCount), status: show.status })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({
				aired: show.kitsuCount,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(show.kitsuCount))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: show.providerCount,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/account/update/:provider`, async ({ request, params }) => {
			writes.push({
				provider: String(params.provider),
				body: (await request.json()) as Record<string, unknown>
			});
			return HttpResponse.json(null);
		})
	);
}

/** The page's stream for `episode` not yet landed, if it has one. The
 *  page warms a neighbour in the background, and a switch to it may
 *  take over that warm stream rather than open its own — so either
 *  may be the one the switch waits on. */
function openStreamFor(episode: number): FakeEventSource | undefined {
	return FakeEventSource.instances
		.filter((i) => !i.landed && new URL(i.url).searchParams.get('episode') === `${episode}`)
		.at(-1);
}

function land(es: FakeEventSource, episode: number, mode: 'sub' | 'dub') {
	es.landed = true;
	es.dispatch(
		'done',
		JSON.stringify({
			id: `session-${episode}-${FakeEventSource.instances.length}`,
			kind: 'hls',
			has_subtitles: false,
			quality: '1080',
			mode,
			episode: `${episode}`
		})
	);
}

/** Mount the play page on episode `from`, then switch to each of
 *  `episodes` in turn, landing every switch. The strip shows the
 *  five-episode page holding the episode playing, and every episode
 *  switched to here is on that page. */
async function watch(show: Show, from: number, ...episodes: number[]) {
	setUrl(`/play/${KITSU_ID}`, { episode: `${from}` });
	serve(show);
	const mode = show.mode ?? 'sub';
	app = mount(PlayPage, { target });
	for (const episode of episodes) {
		await until(() => card(episode) !== null, `the card for episode ${episode}`);
		const before = writes.length;
		card(episode)!.click();
		// Land the switch's stream as it opens; a switch served from a
		// session the page already holds opens none.
		await until(() => {
			const es = openStreamFor(episode);
			if (es) land(es, episode, mode);
			return writes.length >= before + TRACKERS.length;
		}, `both trackers' writes for episode ${episode}`);
	}
}

/** The body each tracker received for its `n`th write. */
function nth(n: number): Record<string, Record<string, unknown>> {
	const out: Record<string, Record<string, unknown>> = {};
	for (const provider of TRACKERS) {
		const own = writes.filter((w) => w.provider === provider);
		out[provider] = own[n]?.body;
	}
	return out;
}

const both = (body: Record<string, unknown>) => ({ mal: body, anilist: body });

describe('play route — what the trackers receive for a watched episode', () => {
	it("Kitsu's last episode of a finished show sends completed to every tracker", async () => {
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 12 }, 11, 12);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 12, status: 'completed' }));
	});

	it("an episode the provider lists past Kitsu's count sends its number without completed", async () => {
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 13 }, 12, 13);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 13 }));
	});

	it('an episode before the finale sends its number alone', async () => {
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 12 }, 12, 11);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 11 }));
	});

	it('the latest episode of a show still airing never completes it', async () => {
		await watch({ kitsuCount: 12, status: 'current', providerCount: 12 }, 11, 12);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 12 }));
	});

	it("the last dubbed episode of a part-dubbed show is not the series' finale", async () => {
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 6, mode: 'dub' }, 7, 6);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 6 }));
	});

	it('watching the finale again sends completed again', async () => {
		// The page cannot see a rewatch: whether the tracker's row is
		// mid-rewatch, and so keeps its status, is read and decided
		// behind the progress route.
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 12 }, 11, 12, 11, 12);
		const finale = both({ kitsu_id: KITSU_ID, progress: 12, status: 'completed' });
		expect(nth(0)).toEqual(finale);
		expect(nth(1)).toEqual(both({ kitsu_id: KITSU_ID, progress: 11 }));
		expect(nth(2)).toEqual(finale);
	});

	it('an earlier episode replayed after a later one sends its own number, with no status', async () => {
		// The page sends the episode it played; leaving a row that is
		// already further along untouched is the backend's guard.
		await watch({ kitsuCount: 12, status: 'finished', providerCount: 12 }, 10, 9, 7);
		expect(nth(0)).toEqual(both({ kitsu_id: KITSU_ID, progress: 9 }));
		expect(nth(1)).toEqual(both({ kitsu_id: KITSU_ID, progress: 7 }));
	});
});
