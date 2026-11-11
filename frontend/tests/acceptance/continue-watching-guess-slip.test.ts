// Acceptance: a guessed Continue card opens a player that asks "Is this
// the show you meant?", whatever its play wrote on the way.
//
// Home shows the card's "Not this show?" pill because its history row
// records no Kitsu id and the show is a guess (here a remembered title
// match). A click opens the play as a guess of that row. When the play
// lands on the row's own listing, the backend records the guess on the
// row — in the resolve's own history write when the cached Kitsu detail
// already answers the cour guard (`play_native_record_id::resolved_id`),
// so before the session reaches the player; otherwise once the watch's
// guard has read Kitsu, usually after the player opened. The cached case
// is the one a profile reaches whenever Home resolved the row through
// its remembered title match, which reads the show's detail into the
// cache. Whichever way the write went, the session is the guess Home
// opened, and its player asks; a guess whose play landed on another
// listing leaves the row a guess and asks too. The show page the player
// goes up to keeps the same answer.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { goto } from '$app/navigation';
import { __resetApiBaseForTests, markWatched } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { __resetHiddenRowsForTests } from '../../src/lib/history/in-place-link.svelte';

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

/** The row's saved title, and the show its title match names. */
const TITLE = 'Gintama Season 5';
const GUESS = '12553';
const GUESS_TITLE = 'Gintama.';
/** The card's own row, and the listing a wrong guess's play lands on. */
const CARD = 'hianime:gintama-season-5-1385';
const ELSEWHERE = 'hianime:gintama-2017-9999';

interface Row {
	ep_no: string;
	id: string;
	title: string;
	kitsu_id?: string;
	guessed_from?: string;
	named?: boolean;
}

let rows: Row[];
let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	vi.mocked(goto).mockClear();
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	__resetHiddenRowsForTests();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
	rows = [{ ep_no: '1', id: CARD, title: TITLE }];
	target = document.createElement('div');
	document.body.appendChild(target);
	serve();
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

const show = () => ({
	...kitsuRef(GUESS, GUESS_TITLE, 51),
	titles: { en_us: TITLE },
	slug: 'gintama-2017'
});

function serve() {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json(rows)),
			http.get(`${API_BASE}/api/history/:id/number`, ({ params }) => {
				const row = rows.find((r) => r.id === params.id);
				return HttpResponse.json(row ? { ep_no: row.ep_no } : null);
			}),
			http.get(`${API_BASE}/api/history/by-kitsu/:id`, ({ params }) =>
				HttpResponse.json(rows.find((r) => r.kitsu_id === params.id) ?? null)
			),
			// Continue Watching remembers the title's guess.
			http.get(`${API_BASE}/api/title-match`, ({ request }) =>
				HttpResponse.json(new URL(request.url).searchParams.get('title') === TITLE ? GUESS : null)
			),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([show()])),
			http.get(`${API_BASE}/api/kitsu/anime/:id`, () => HttpResponse.json(show())),
			http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
				HttpResponse.json({ aired: 51, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 51,
					extra_episodes: [],
					episode_count_approximate: false
				})
			),
			http.post(`${API_BASE}/api/history/link-group`, async ({ request }) => {
				const body = (await request.json()) as { card: string; rows: string[] };
				return HttpResponse.json(body.rows.filter((r) => r === body.card));
			}),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/account/entry/:provider`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/account/list/:provider/cached`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json(null))
		])
	);
}

const card = () =>
	Array.from(target.querySelectorAll<HTMLButtonElement>('button.resume-card')).find((b) =>
		(b.textContent ?? '').includes(GUESS_TITLE)
	) ?? null;
const pill = () => target.querySelector<HTMLButtonElement>('button.resume-link');
const slip = () => target.querySelector<HTMLElement>('[data-testid="guess-slip"]');
const asks = () => (slip()?.textContent ?? '').includes(m.play_link_guess_note({ title: TITLE }));

/** How the backend's writes go as the card's play lands. */
type Landing =
	/** On the card's own listing, the guess recorded on its row by the
	 *  resolve, before the session answers: the cached Kitsu detail
	 *  answered the cour guard. */
	| 'own-recorded'
	/** On the card's own listing, the guess not yet on the row when the
	 *  player reads it: the guard is still reading Kitsu. */
	| 'own-pending'
	/** On another listing: the play writes that listing's row, noting
	 *  the card it came from, and the card's row stays a guess. */
	| 'elsewhere';

/** Home with its guessed card's pill, the card clicked and its play
 *  landed as `landing` says, then the player at the URL Home opened. */
async function playFromHome(landing: Landing): Promise<URL> {
	setUrl('/');
	app = mount(HomePage, { target });
	await until(() => card() !== null, 'the Continue card');
	// Home offers its pill: the card is a guess.
	await until(() => pill() !== null, "the card's pill");
	card()!.click();
	await until(() => FakeEventSource.instances.length > 0, "the card's play");
	const stream = FakeEventSource.instances[0];
	expect(new URL(stream.url).searchParams.get('guess_row')).toBe(CARD);
	if (landing === 'own-recorded') {
		rows = rows.map((r) => (r.id === CARD ? { ...r, kitsu_id: GUESS } : r));
	} else if (landing === 'elsewhere') {
		rows = [{ ep_no: '2', id: ELSEWHERE, title: TITLE, guessed_from: CARD }, ...rows];
	}
	stream.dispatch(
		'done',
		JSON.stringify({
			session_id: 's-1',
			media_url: `${API_BASE}/s/s-1/file.mp4`,
			media_kind: 'mp4',
			...(landing === 'elsewhere' ? {} : { own_listing: true })
		})
	);
	await until(() => vi.mocked(goto).mock.calls.length > 0, 'the player to open');
	const href = new URL(String(vi.mocked(goto).mock.calls.at(-1)?.[0]), 'http://127.0.0.1:31337');
	expect(href.searchParams.get('guess')).toBe('1');
	expect(href.searchParams.get('row')).toBe(CARD);
	unmount(app);
	app = null;
	FakeEventSource.instances.length = 0;
	const id = href.pathname.split('/').at(-1)!;
	setParams({ id });
	setUrl(href.pathname, Object.fromEntries(href.searchParams));
	app = mount(PlayPage, { target });
	return href;
}

describe('the player a guessed Continue card opens', () => {
	it.each<Landing>(['own-recorded', 'own-pending', 'elsewhere'])(
		'asks whether this is the show meant (play landed %s)',
		async (landing) => {
			await playFromHome(landing);
			await until(asks, 'the slip asking');
			expect(Array.from(slip()!.querySelectorAll('button')).length).toBeGreaterThan(0);
		}
	);
});

describe('the show page that player goes up to', () => {
	it.each<Landing>(['own-recorded', 'own-pending'])(
		'keeps the quiet line (play landed %s)',
		async (landing) => {
			await playFromHome(landing);
			await until(() => target.querySelector('a.np-link') !== null, 'the now-playing link');
			const up = target.querySelector<HTMLAnchorElement>('a.np-link')!.getAttribute('href')!;
			unmount(app!);
			app = null;
			const next = new URL(up, 'http://127.0.0.1:31337');
			setParams({ id: next.pathname.split('/').at(-1)! });
			setUrl(next.pathname, Object.fromEntries(next.searchParams));
			app = mount(DetailPage, { target });
			await until(
				() => (target.textContent ?? '').includes(m.detail_link_matched({ title: TITLE })),
				'the matched-from line'
			);
		}
	);
});

describe('a guessed URL visited again', () => {
	it('asks nothing once the show was played from its own page, which recorded it', async () => {
		const href = await playFromHome('own-recorded');
		await until(asks, 'the slip asking');
		unmount(app!);
		app = null;
		// The show played from its own page: its watch names the show and
		// no card.
		await markWatched({ title: GUESS_TITLE, episode: '2', mode: 'sub', kitsu_id: GUESS });
		// Back to the guessed player's URL.
		setParams({ id: GUESS });
		setUrl(href.pathname, Object.fromEntries(href.searchParams));
		app = mount(PlayPage, { target });
		await until(() => target.querySelector('a.np-link') !== null, 'the now-playing link');
		await new Promise((r) => setTimeout(r, 300));
		expect(asks()).toBe(false);
	});
});
