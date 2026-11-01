// Acceptance: a guessed Continue card's play binds the show when it
// lands on the card's own listing.
//
// A Continue card whose show is only a guess opens its play as a guess
// and names the card's history row on it (`guess_row`). The backend
// answers whether the play landed on that row's own listing
// (`own_listing`: the same show key, or for a bare row from a retired
// provider the same slug words before its number — decided and tested
// in `backend/src/scraper/show_key.rs`). When it did, the watch is
// stamped with the guessed Kitsu id and written to the trackers, as a
// play from the show's page is; otherwise neither.
//
// What the stamp then does to the history — the played row gains the
// id and the show's mapping, so the card is matched by it and the
// show's page finds the row — is the backend's
// (`backend/src/api/mark_watched_guess_test.rs`, and the listing check
// it makes on the stamp). The fake backend below only records the
// watch on the listing played under the id the stamp carries, and
// finds a row for the show's page by its recorded id or its stored
// mapping, as `history_by_kitsu` does. The rows here start with no
// stored mapping — a guess from a title search stores none — so the
// show's page has no row for them until a play binds one.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';

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
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { goto } from '$app/navigation';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests, getOrFire, makeKey } from '../../src/lib/play/play-cache';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { Provider } from '../../src/lib/account/types';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {}
	dispatch(name: string, data: string) {
		const ev = { data, type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

const SHOW = 'Cowboy Bebop';
const KITSU_ID = '1';
/** The listing the provider plays for the guessed show. */
const LISTING = 'hianime:cowboy-bebop-1';
const TRACKERS: Provider[] = ['mal', 'anilist'];

interface Row {
	ep_no: string;
	id: string;
	title: string;
	kitsu_id?: string;
}

/** The fake backend's state: its history rows, what each watch stamp
 *  asked to record, and what each tracker received. */
let rows: Row[];
/** The show key → Kitsu id mappings the fake backend holds. */
let mappings: Record<string, string>;
let stamps: Record<string, unknown>[];
let trackerWrites: string[];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	stamps = [];
	mappings = {};
	trackerWrites = [];
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

/** The backend the routes talk to, over `rows`. A watch stamp records
 *  the watch on the listing played, under the Kitsu id it carries —
 *  the contract `mark_watched_guess_test.rs` holds the backend to. */
function serveBackend() {
	const show = () => ({ ...kitsuRef(KITSU_ID, SHOW, 26), status: 'finished' });
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json(rows)),
			http.get(`${API_BASE}/api/history/:id/number`, ({ params }) => {
				const row = rows.find((r) => r.id === params.id);
				return HttpResponse.json(row ? { ep_no: row.ep_no } : null);
			}),
			// A row by its recorded id, else a row with none by its mapping.
			http.get(`${API_BASE}/api/history/by-kitsu/:id`, ({ params }) =>
				HttpResponse.json(
					rows.find((r) => r.kitsu_id === params.id) ??
						rows.find((r) => !r.kitsu_id && mappings[r.id] === params.id) ??
						null
				)
			),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([show()])),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () => HttpResponse.json(show())),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 26,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
				const body = (await request.json()) as Record<string, unknown>;
				stamps.push(body);
				const kitsuId = typeof body.kitsu_id === 'string' ? body.kitsu_id : undefined;
				const played: Row = { ep_no: String(body.episode), id: LISTING, title: SHOW };
				const kept = rows.find((r) => r.id === LISTING);
				if (kitsuId) mappings[LISTING] = kitsuId;
				rows = [
					{
						...played,
						...(kitsuId ? { kitsu_id: kitsuId } : kept?.kitsu_id ? { kitsu_id: kept.kitsu_id } : {})
					},
					...rows.filter((r) => r.id !== LISTING)
				];
				return new HttpResponse(null, { status: 204 });
			}),
			http.post(`${API_BASE}/api/account/update/:provider`, ({ params }) => {
				trackerWrites.push(String(params.provider));
				return HttpResponse.json(null);
			}),
			// The show page's list editor reads each tracker's entry and list.
			http.get(`${API_BASE}/api/account/entry/:provider`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/account/list/:provider/cached`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null))
		])
	);
}

const card = () =>
	Array.from(target.querySelectorAll('button')).find((b) => b.textContent?.includes(SHOW)) ?? null;

/** Click the card, land its play as the backend answers it, and return
 *  the play's stream URL and the URL the session opens at. */
async function playTheCard(ownListing: boolean): Promise<{ stream: URL; session: URL }> {
	app = mount(HomePage, { target });
	await until(() => card() !== null, 'the Continue card');
	card()!.click();
	await until(() => FakeEventSource.instances.length > 0, "the card's play stream");
	const stream = FakeEventSource.instances[0];
	stream.dispatch(
		'done',
		JSON.stringify({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4',
			...(ownListing ? { own_listing: true } : {})
		})
	);
	await until(() => vi.mocked(goto).mock.calls.length > 0, 'the play page to open');
	await until(() => stamps.length > 0, 'the watch to be stamped');
	const session = new URL(String(vi.mocked(goto).mock.calls.at(-1)?.[0]), 'http://app.test');
	unmount(app);
	app = null;
	return { stream: new URL(stream.url), session };
}

/** On a later visit, the card's play: whether it is still a guess, and
 *  the Kitsu id it asks to record. */
async function playTheCardAgain(): Promise<{ guess: boolean; recorded: string | null }> {
	vi.mocked(goto).mockClear();
	FakeEventSource.instances.length = 0;
	__resetPlayCacheForTests();
	const { stream, session } = await playTheCard(false);
	return {
		guess: session.searchParams.get('guess') === '1',
		recorded: stream.searchParams.get('kitsu_id')
	};
}

/** The show's page, and whether its Play button continues the history
 *  row it finds rather than starting from episode 1. */
async function showPageResumes(): Promise<boolean> {
	setParams({ id: KITSU_ID });
	app = mount(DetailPage, { target });
	const resume = m.detail_play_button_resume({ episode: '' }).trim();
	const start = m.detail_play_button_episode_one();
	const button = (label: string) =>
		Array.from(target.querySelectorAll('button')).some((b) =>
			(b.textContent ?? '').includes(label)
		);
	await until(() => button(resume) || button(start), "the show page's Play button");
	return button(resume);
}

describe('a guessed Continue card whose play lands on its own listing', () => {
	it('records the show and writes the trackers; the card and the show page then know it', async () => {
		rows = [{ ep_no: '3', id: LISTING, title: SHOW }];
		serveBackend();

		const { stream, session } = await playTheCard(true);
		expect(stream.searchParams.get('guess_row')).toBe(LISTING);
		expect(session.searchParams.get('guess')).toBe('1');
		expect(stamps[0]).toMatchObject({ kitsu_id: KITSU_ID, episode: '4' });
		await until(() => trackerWrites.length >= TRACKERS.length, 'the tracker writes');
		expect([...trackerWrites].sort()).toEqual([...TRACKERS].sort());

		expect(await playTheCardAgain()).toEqual({ guess: false, recorded: KITSU_ID });
		expect(await showPageResumes()).toBe(true);
	});
});

describe('a guessed Continue card whose play lands on another listing', () => {
	it('records no show and writes no tracker; the card stays a guess', async () => {
		rows = [{ ep_no: '3', id: LISTING, title: SHOW }];
		serveBackend();

		const { stream } = await playTheCard(false);
		expect(stream.searchParams.get('guess_row')).toBe(LISTING);
		expect(stamps[0]).not.toHaveProperty('kitsu_id');
		// A tracker write leaves beside the stamp; one more turn, then none.
		await new Promise((r) => setTimeout(r, 0));
		expect(trackerWrites).toEqual([]);

		expect(await playTheCardAgain()).toEqual({ guess: true, recorded: null });
		expect(await showPageResumes()).toBe(false);
	});
});

describe('a guessed Continue card beside a session the show page warmed', () => {
	it("plays its own request, which can bind, rather than the show page's", async () => {
		// The show page warmed the card's next episode under the show's
		// id, with no card's row behind it: that session never answers
		// whether it landed on this card's listing.
		rows = [{ ep_no: '3', id: LISTING, title: SHOW }];
		serveBackend();
		// Settled before the card exists: a warm still running would be
		// taken over by the click's own request, which proves nothing.
		await getOrFire(makeKey(KITSU_ID, 4, 'sub', 'best'), async () => ({
			session_id: 'warm',
			media_url: `${API_BASE}/s/warm/file.mp4`,
			media_kind: 'mp4' as const
		}));

		const { stream } = await playTheCard(true);
		expect(stream.searchParams.get('guess_row')).toBe(LISTING);
		expect(stamps[0]).toMatchObject({ kitsu_id: KITSU_ID });
	});
});

describe('an old card from a retired provider', () => {
	it('names its bare row on the play, and binds on a yes', async () => {
		// Whether its slug words match the listing played is the
		// backend's answer (`same_listing`); here it says yes.
		const BARE = 'cowboy-bebop-77';
		rows = [{ ep_no: '3', id: BARE, title: SHOW }];
		serveBackend();

		const { stream } = await playTheCard(true);
		expect(stream.searchParams.get('guess_row')).toBe(BARE);
		expect(stamps[0]).toMatchObject({ kitsu_id: KITSU_ID });
		await until(() => trackerWrites.length >= TRACKERS.length, 'the tracker writes');
	});
});
