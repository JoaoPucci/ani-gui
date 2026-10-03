// Acceptance: a play page that has gone sends nobody back to it.
//
// An episode switch and a stale-stream recovery both await a session
// and then navigate to /play. If the page is left while one is
// pending, the answer arriving afterwards must not navigate anywhere
// or start resolving again: the viewer left, and being pulled back
// into the player from wherever they went is the one thing worse
// than the stream not recovering.

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
	__resetPlayCacheForTests();
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

function useShowHandlers(evict: () => Promise<Response> | Response) {
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
		http.post(`${API_BASE}/api/play/cache/evict`, evict),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

const session = (id: string) =>
	JSON.stringify({ id, kind: 'mp4', has_subtitles: false, quality: '1080', mode: 'sub' });

describe('play route — a page that has gone sends nobody back to it', () => {
	it('a recovery still evicting when the page is left resolves nothing and goes nowhere', async () => {
		let releaseEvict: () => void = () => {};
		let evictAsked = false;
		useShowHandlers(async () => {
			evictAsked = true;
			await new Promise<void>((r) => {
				releaseEvict = r;
			});
			return new HttpResponse(null, { status: 204 });
		});
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');

		const video = playerVideo();
		video.currentTime = 720;
		Object.defineProperty(video, 'error', { value: { code: 2 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await until(() => evictAsked, 'the recovery to evict the stale row');

		unmount(app);
		app = null;
		const streamsAtLeave = FakeEventSource.instances.length;
		releaseEvict();
		await new Promise((r) => setTimeout(r, 200));

		expect(FakeEventSource.instances.length).toBe(streamsAtLeave);
		expect(goto).not.toHaveBeenCalled();
	});

	it('a switch whose session is in hand when the page is left goes nowhere', async () => {
		useShowHandlers(() => new HttpResponse(null, { status: 204 }));
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		// The next episode's warm lands, so the click below has its
		// session at once and only an await stands between it and
		// the navigation.
		await until(
			() => FakeEventSource.instances.some((i) => i.url.includes('episode=2')),
			'the next-episode warm stream'
		);
		FakeEventSource.instances
			.filter((i) => i.url.includes('episode=2'))
			.at(-1)!
			.dispatch('done', session('session-2'));
		await new Promise((r) => setTimeout(r, 20));

		const tile = target.querySelector('li[data-ep-num="2"] button') as HTMLButtonElement;
		expect(tile).not.toBeNull();
		tile.click();
		unmount(app);
		app = null;
		await new Promise((r) => setTimeout(r, 100));

		expect(goto).not.toHaveBeenCalled();
	});
});
