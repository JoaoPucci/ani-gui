// Acceptance: the play page's handoffs — the external player and
// Syncplay — send the request the embedded play sends, the Kitsu id
// included.
//
// The id is what lets the backend persist the show's reverse mapping
// under the handoff's history row, so the detail page can find that
// row and weigh it against another provider's. The request builder
// has its own unit tests; this drives the mounted page: open the
// "More actions" menu under the player, click each handoff, and read
// the body the page posted.
//
// The stream is an mp4 session, as in the mute-shortcut spec: an HLS
// one would ask hls.js for a MediaSource happy-dom does not have.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
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

import { goto } from '$app/navigation';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

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
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	setParams({ id: KITSU_ID });
	setUrl(`/play/${KITSU_ID}`, { episode: '3' });
	target = document.createElement('div');
	document.body.appendChild(target);
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

type Posted = Record<string, unknown>;

/** Stubs every request the page makes on mount, and records the body
 *  of each handoff. */
function useHandlers(posted: { external: Posted[]; syncplay: Posted[] }) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
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
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/play/external`, async ({ request }) => {
			posted.external.push((await request.json()) as Posted);
			return new HttpResponse(null, { status: 202 });
		}),
		http.post(`${API_BASE}/api/play/syncplay`, async ({ request }) => {
			posted.syncplay.push((await request.json()) as Posted);
			return new HttpResponse(null, { status: 202 });
		})
	);
}

/** Mount the page and settle it on an mp4 session for episode 3. */
async function mountPlaying() {
	app = mount(PlayPage, { target });
	await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
	FakeEventSource.instances[0].dispatch(
		'done',
		JSON.stringify({
			id: 'session-1',
			kind: 'mp4',
			has_subtitles: false,
			quality: '1080',
			mode: 'sub'
		})
	);
	// The page carries the session in the URL and gets there by
	// `goto`, which this tier stubs out — so the stub is moved by
	// hand to where the real navigation would have landed.
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4' });
	await until(() => moreButton() !== null, 'the More actions button under the player');
}

const moreButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_more_aria_label()}"]`
	) as HTMLButtonElement | null;

const menuItem = (label: string) =>
	Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
		(b.textContent ?? '').includes(label)
	) as HTMLButtonElement | undefined;

async function clickMenuItem(label: string) {
	moreButton()!.click();
	await until(() => menuItem(label) !== undefined, `the "${label}" menu item`);
	menuItem(label)!.click();
}

/** What both handoffs must send: the embedded play's request for
 *  the episode on screen, plus the Kitsu id the page was opened for. */
function expectHandoffBody(body: Posted) {
	expect(body.kitsu_id).toBe(KITSU_ID);
	expect(body.title).toBe(TITLE);
	expect(body.episode).toBe('3');
	expect(body.mode).toBe('sub');
	expect(body.episode_count).toBe(12);
}

describe('play route — the handoffs send the Kitsu id', () => {
	it('the external-player handoff posts the episode with the Kitsu id', async () => {
		const posted = { external: [] as Posted[], syncplay: [] as Posted[] };
		useHandlers(posted);
		await mountPlaying();

		await clickMenuItem(m.play_external_label());
		await until(() => posted.external.length === 1, 'the external-player request');
		expectHandoffBody(posted.external[0]);
		expect(posted.syncplay).toHaveLength(0);
	});

	it('the Syncplay handoff posts the episode with the Kitsu id', async () => {
		const posted = { external: [] as Posted[], syncplay: [] as Posted[] };
		useHandlers(posted);
		await mountPlaying();

		await clickMenuItem(m.play_hamburger_syncplay_label());
		await until(() => posted.syncplay.length === 1, 'the Syncplay request');
		expectHandoffBody(posted.syncplay[0]);
		expect(posted.external).toHaveLength(0);
	});
});

// A session opened from a Continue card whose match was only a guess
// carries the flag in its URL, and records the guess nowhere: the next
// request would otherwise pin it on the history row, as the card's
// click would have. The verdict lives with the session, so it holds
// whatever the home page does meanwhile, and a session without the
// flag — one opened from the detail page — records its id as before.
describe('play route — a session opened from a guess records no Kitsu id', () => {
	it('the stream and the external-player handoff carry no Kitsu id', async () => {
		setUrl(`/play/${KITSU_ID}`, { episode: '3', guess: '1' });
		const posted = { external: [] as Posted[], syncplay: [] as Posted[] };
		useHandlers(posted);
		app = mount(PlayPage, { target });
		await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({
				id: 'session-1',
				kind: 'mp4',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4', guess: '1' });
		await until(() => moreButton() !== null, 'the More actions button under the player');

		expect(new URL(FakeEventSource.instances[0].url).searchParams.get('kitsu_id')).toBeNull();
		await clickMenuItem(m.play_external_label());
		await until(() => posted.external.length === 1, 'the external-player request');
		expect(posted.external[0].kitsu_id).toBeUndefined();
		expect(posted.external[0].title).toBe(TITLE);
	});

	it('an episode switch keeps the guess: its stream, watch and URL carry no Kitsu id', async () => {
		setUrl(`/play/${KITSU_ID}`, { episode: '3', guess: '1' });
		const posted = { external: [] as Posted[], syncplay: [] as Posted[] };
		useHandlers(posted);
		const watched: Posted[] = [];
		server.use(
			http.post(`${API_BASE}/api/play/mark-watched`, async ({ request }) => {
				watched.push((await request.json()) as Posted);
				return new HttpResponse(null, { status: 204 });
			})
		);
		app = mount(PlayPage, { target });
		await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({ id: 'session-1', kind: 'mp4', has_subtitles: false })
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '3', kind: 'mp4', guess: '1' });
		const next = () =>
			target.querySelector(
				`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
			) as HTMLButtonElement | null;
		await until(() => next() !== null && !next()!.disabled, 'the next-episode button');
		vi.mocked(goto).mockClear();

		next()!.click();
		await until(() => FakeEventSource.instances.length > 1, 'the switch stream');
		const switchUrl = new URL(FakeEventSource.instances[1].url);
		expect(switchUrl.searchParams.get('episode')).toBe('4');
		expect(switchUrl.searchParams.get('kitsu_id')).toBeNull();
		FakeEventSource.instances[1].dispatch(
			'done',
			JSON.stringify({ id: 'session-2', kind: 'mp4', has_subtitles: false })
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the switch navigation');

		const landed = new URL(String(vi.mocked(goto).mock.calls[0][0]), 'http://x');
		expect(landed.searchParams.get('episode')).toBe('4');
		expect(landed.searchParams.get('guess')).toBe('1');
		await until(() => watched.some((w) => w.episode === '4'), 'the switch watch');
		expect(watched.find((w) => w.episode === '4')?.kitsu_id).toBeUndefined();
	});
});
