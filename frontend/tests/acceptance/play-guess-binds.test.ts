// Acceptance: a play page opened from a guessed Continue card binds
// each episode it plays on the card's own listing.
//
// A guessed card's session carries the card's history row in its URL
// (`row`, beside `guess`). Every episode switch names that row on its
// play (`guess_row`), and the backend answers whether the play landed
// on the row's own listing (`own_listing`). A switch that did is
// stamped with the show's Kitsu id and written to every connected
// tracker, as a play from the show's page is; one that landed
// elsewhere is stamped with no id and written nowhere. The Home click
// that opened the session is covered in
// `continue-watching-guess-binds.test.ts`.

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
let marked: string[];
let stamped: { episode?: string; kitsu_id?: string }[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	writes = [];
	marked = [];
	stamped = [];
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

/** Long enough for a tracker write the stamp set off to arrive: the
 *  write goes out after a few awaited steps, which the positive cases
 *  here see land well inside this. */
const noTrackerWriteWindow = () => new Promise((r) => setTimeout(r, 300));

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
		http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
			const body = (await request.json()) as { episode?: string; kitsu_id?: string };
			marked.push(String(body.episode));
			stamped.push(body);
			return new HttpResponse(null, { status: 204 });
		}),
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

function land(es: FakeEventSource, episode: number, unverified = false, ownListing = false) {
	es.landed = true;
	es.dispatch(
		'done',
		JSON.stringify({
			session_id: `session-${episode}-${FakeEventSource.instances.length}`,
			media_url: `${API_BASE}/s/session-${episode}-${FakeEventSource.instances.length}/master.m3u8`,
			media_kind: 'hls',
			episode: `${episode}`,
			...(unverified ? { episode_unverified: true } : {}),
			...(ownListing ? { own_listing: true } : {})
		})
	);
}

const ROW = 'hianime:finished-show-7';

/** Open a guessed card's session on episode `from`, switch to `to`,
 *  and land the switch as the backend answers it. */
async function switchInGuessedSession(
	from: number,
	to: number,
	ownListing: boolean,
	unverified = false
) {
	const show = { kitsuCount: 12, status: 'finished', providerCount: 12 };
	setUrl(`/play/${KITSU_ID}`, { episode: `${from}`, guess: '1', row: ROW });
	serve(show);
	app = mount(PlayPage, { target });
	await until(() => card(to) !== null, `the card for episode ${to}`);
	card(to)!.click();
	let asked: URL | null = null;
	await until(() => {
		const es = openStreamFor(to);
		if (es) {
			asked = new URL(es.url);
			land(es, to, unverified, ownListing);
		}
		return marked.includes(`${to}`);
	}, `episode ${to} marked watched`);
	return asked as unknown as URL;
}

describe("play route — a guessed card's session", () => {
	it("binds a switch that lands on the card's own listing, and writes the trackers", async () => {
		const asked = await switchInGuessedSession(3, 4, true);
		expect(asked.searchParams.get('guess_row')).toBe(ROW);
		expect(asked.searchParams.has('kitsu_id')).toBe(false);
		expect(stamped.at(-1)).toMatchObject({ episode: '4', kitsu_id: KITSU_ID });
		await until(() => writes.length >= TRACKERS.length, 'both trackers written');
		for (const provider of TRACKERS) {
			expect(writes.find((w) => w.provider === provider)?.body).toMatchObject({
				kitsu_id: KITSU_ID,
				progress: 4
			});
		}
	});

	it('records no id and writes no tracker for a switch that landed elsewhere', async () => {
		const asked = await switchInGuessedSession(3, 4, false);
		expect(asked.searchParams.get('guess_row')).toBe(ROW);
		expect(stamped.at(-1)).not.toHaveProperty('kitsu_id');
		await noTrackerWriteWindow();
		expect(writes).toEqual([]);
	});

	it('binds a switch it could not verify, and writes no tracker for it', async () => {
		const asked = await switchInGuessedSession(3, 4, true, true);
		expect(asked.searchParams.get('guess_row')).toBe(ROW);
		expect(stamped.at(-1)).toMatchObject({ episode: '4', kitsu_id: KITSU_ID });
		await noTrackerWriteWindow();
		expect(writes).toEqual([]);
	});
});
