// Acceptance: the player's loading indicator never sits over its
// error.
//
// The indicator shows while an episode switch is on its way. A
// playback error can land inside that window — the old stream is
// still attached, and a switch in flight rules out the automatic
// recovery — and the frame then shows the error in place of the
// picture. From that point the error is what the frame says; the
// indicator steps back rather than drawing over it.

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
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

import { goto } from '$app/navigation';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { playerVideo, playerVideoInSlot } from './player-video';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

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
	__resetPlayCacheForTests();
	window.localStorage.clear();
	vi.mocked(goto).mockClear();
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	setParams({ id: KITSU_ID });
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

function useShowHandlers() {
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
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

// The indicator stays mounted across a switch and the hold that
// follows it and is toggled on and off, so a short gap between the two
// never restarts it; `player-spinner-on` is its showing state. An
// error unmounts it outright.
const indicator = () => target.querySelector('.player-spinner');
const indicatorOn = () => target.querySelector('.player-spinner.player-spinner-on');
const errorPanel = () => target.querySelector('.player-error');

const session = (id: string) =>
	JSON.stringify({ id, kind: 'mp4', has_subtitles: false, quality: '1080', mode: 'sub' });
const busy = () =>
	target.querySelector('section.player-frame')?.getAttribute('aria-busy') === 'true';

describe('play route — the loading indicator never sits over the error', () => {
	it('a playback error during a switch replaces the indicator', async () => {
		useShowHandlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		await until(
			() => target.querySelector('li[data-ep-num="3"] button') !== null,
			'the episode strip'
		);

		// The switch's stream never answers, so the switch stays on
		// its way for the rest of the scenario.
		(target.querySelector('li[data-ep-num="3"] button') as HTMLButtonElement).click();
		await until(() => indicatorOn() !== null, 'the indicator during the switch');

		const video = playerVideo();
		Object.defineProperty(video, 'error', { value: { code: 3 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await until(() => errorPanel() !== null, 'the error in place of the picture');

		expect(indicator()).toBeNull();
	});
});

describe('play route — the loading indicator runs from a switch into the hold after it', () => {
	it('a switch stays busy until its navigation has landed', async () => {
		// The next episode's stream attaches, and a resume hold starts,
		// when the navigation lands. A switch that stopped being busy
		// as soon as it asked for the navigation left a moment between
		// the two with nothing loading, and the indicator went out in
		// it.
		useShowHandlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=2')),
			'the next-episode warm stream'
		);
		FakeEventSource.instances
			.filter((i) => i.url.includes('episode=2'))
			.at(-1)!
			.dispatch('done', session('session-2'));
		await new Promise((r) => setTimeout(r, 20));

		let land: () => void = () => {};
		vi.mocked(goto).mockImplementationOnce(
			() =>
				new Promise<void>((r) => {
					land = r;
				})
		);
		(target.querySelector('li[data-ep-num="2"] button') as HTMLButtonElement).click();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the switch to navigate');
		await new Promise((r) => setTimeout(r, 20));
		expect(busy()).toBe(true);
		expect(indicatorOn()).not.toBeNull();

		land();
		await until(() => !busy(), 'the switch to finish once the navigation lands');
		expect(indicatorOn()).toBeNull();
	});
});
