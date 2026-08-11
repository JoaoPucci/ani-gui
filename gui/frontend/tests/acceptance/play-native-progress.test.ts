// Acceptance: the native resolver's structured progress kinds render
// through Paraglide on the player page.
//
// The walk fabricates its progress lines and ships them as kinds
// with interpolation data (searching carries the provider, matched
// the title); the overlay must resolve them to the locale's own
// copy. Unit tests pin progressLabel; this drives a real EventSource
// event into the mounted page — the flow the finding named.

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
	setUrl(`/play/${KITSU_ID}`, { episode: '1' });
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

describe('play route — native progress through Paraglide', () => {
	it('renders searching and matched events in the switching overlay', async () => {
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
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		app = mount(PlayPage, { target });

		// The page loads with ?episode=1, whose own resolve opens the
		// first stream — settle it so the switch below owns the
		// overlay.
		await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({
				id: 'session-1',
				kind: 'hls',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		await until(() => card(5) !== null, 'the card for episode 5');

		card(5)!.click();
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=5')),
			'a stream asking for episode 5'
		);
		// The player warms its neighbour in the background with a
		// no-op progress sink, so the switch's own stream is the one
		// asking for episode 5.
		const es = FakeEventSource.instances.find((i) => i.url.includes('episode=5'))!;
		expect(es).toBeTruthy();

		es.dispatch('progress', JSON.stringify({ kind: 'searching', provider: 'anidb.app' }));
		await until(
			() =>
				(target.textContent ?? '').includes(m.play_progress_searching({ provider: 'anidb.app' })),
			'the localized searching caption'
		);

		es.dispatch('progress', JSON.stringify({ kind: 'matched', title: TITLE }));
		await until(
			() => (target.textContent ?? '').includes(m.play_progress_matched({ title: TITLE })),
			'the localized matched caption'
		);

		es.dispatch(
			'done',
			JSON.stringify({
				id: 'session-2',
				kind: 'hls',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
	});
});
