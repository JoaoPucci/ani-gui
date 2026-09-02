// Acceptance: the stale-stream recovery announces itself and hands
// playback back where it stood.
//
// A mid-stream network error triggers the silent recovery. The user
// must see a toast naming what is happening (the black frame used to
// be unexplained), and once the fresh session attaches, playback
// must resume from where it stalled instead of restarting at zero.
//
// The toast surface renders in the layout, which this tier does not
// mount, so the assertion reads the toast store the surface renders
// from. The stream is an mp4 session — happy-dom has no MediaSource —
// and the video error is simulated the way Chromium reports it:
// MediaError code 2 on the element plus an `error` event.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
import { toastStore } from '../../src/lib/toasts/store.svelte';

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
import { getGlobalVideo } from '../../src/lib/play/global-video';

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

describe('play route — the stale-stream recovery is visible and lossless', () => {
	it('toasts the recovery and resumes playback where it stalled', async () => {
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
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });

		const video = getGlobalVideo();
		await until(
			() => video.parentElement?.classList.contains('player-video-slot') === true,
			'the video in its slot'
		);
		// The show detail must have landed before the recovery can run;
		// the episode-nav cluster renders once it has.
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');

		// Twelve minutes in, the stream dies the way Chromium reports a
		// network failure mid-play.
		video.currentTime = 720;
		Object.defineProperty(video, 'error', { value: { code: 2 }, configurable: true });
		const toastsBefore = toastStore.items.length;
		const streamsBefore = FakeEventSource.instances.length;
		video.dispatchEvent(new Event('error'));

		// The silent recovery announces itself…
		await until(() => toastStore.items.length > toastsBefore, 'the recovery toast');
		const toast = toastStore.items[toastStore.items.length - 1];
		expect(toast.message).toBe(m.play_stall_toast_link_stale());
		expect(toast.kind).toBe('info');

		// …and re-resolves. The fresh session lands the way the real
		// navigation would: done on the recovery's stream, then the
		// session URL (goto is stubbed in this tier).
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the recovery resolve stream'
		);
		FakeEventSource.instances[FakeEventSource.instances.length - 1].dispatch(
			'done',
			JSON.stringify({
				id: 'session-2',
				kind: 'mp4',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '1', kind: 'mp4' });

		// The re-attach starts at zero; metadata arriving is when the
		// page can seek. happy-dom never fires loadedmetadata on its
		// own, so the test fires what the browser would.
		await until(() => video.src.includes('session-2'), 'the fresh session to attach');
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		await until(() => video.currentTime === 720, 'playback to resume where it stalled');
	});

	it('a superseded attach cancels its pending resume seek', async () => {
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
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });

		const video = getGlobalVideo();
		await until(
			() => video.parentElement?.classList.contains('player-video-slot') === true,
			'the video in its slot'
		);
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');

		// A recovery captures the position and its fresh session
		// attaches, arming the pending seek…
		video.currentTime = 720;
		Object.defineProperty(video, 'error', { value: { code: 2 }, configurable: true });
		const streamsBefore = FakeEventSource.instances.length;
		video.dispatchEvent(new Event('error'));
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the recovery resolve stream'
		);
		FakeEventSource.instances[FakeEventSource.instances.length - 1].dispatch(
			'done',
			JSON.stringify({
				id: 'session-2',
				kind: 'mp4',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '1', kind: 'mp4' });
		await until(() => video.src.includes('session-2'), 'the recovered session to attach');

		// …but before its metadata lands, the user moves on and a
		// different session attaches. The armed seek belongs to the
		// superseded attach and must not fire into this one.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-3', episode: '2', kind: 'mp4' });
		await until(() => video.src.includes('session-3'), 'the superseding session to attach');
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		await new Promise((r) => setTimeout(r, 100));
		expect(video.currentTime).toBe(0);
	});
});
