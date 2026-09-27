// Acceptance: a network failure while the player still has buffered
// media in hand is held, not recovered from.
//
// The decision has its own specs; this drives the page wiring — the
// mocked engine reports the fatal the way hls.js does while the video
// element reports minutes of buffered media — and the assertions are
// what the user experiences: nothing shown, no session swap, and the
// engine asked to load again after a delay. With the buffer nearly out
// the same failure takes the recovery it always did.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { HOLD_DELAYS_MS } from '../../src/lib/play/stall-machine';

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

vi.mock('hls.js', () => {
	type Handler = (event: string, data: unknown) => void;
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = { ERROR: 'hlsError', FRAG_LOADED: 'hlsFragLoaded' };
		static isSupported() {
			return true;
		}
		handlers: Record<string, Handler[]> = {};
		startLoadCalls = 0;
		constructor() {
			FakeHls.instances.push(this);
		}
		loadSource() {}
		attachMedia() {}
		on(event: string, handler: Handler) {
			(this.handlers[event] ??= []).push(handler);
		}
		startLoad() {
			this.startLoadCalls += 1;
		}
		stopLoad() {}
		destroy() {}
		emit(event: string, data: unknown) {
			for (const h of this.handlers[event] ?? []) h(event, data);
		}
	}
	return { default: FakeHls };
});

import Hls from 'hls.js';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { getGlobalVideo } from '../../src/lib/play/global-video';

type FakeHlsT = InstanceType<typeof Hls> & {
	startLoadCalls: number;
	emit: (event: string, data: unknown) => void;
};
const hlsInstances = () => (Hls as unknown as { instances: FakeHlsT[] }).instances;
const ERROR = (Hls as unknown as { Events: { ERROR: string } }).Events.ERROR;

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
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	hlsInstances().length = 0;
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

/** The element reports one buffered run from zero to `end`, and the
 *  playhead sits at `at`. happy-dom's own `buffered` is empty. */
function bufferedTo(video: HTMLVideoElement, end: number, at: number) {
	Object.defineProperty(video, 'buffered', {
		configurable: true,
		get: () => ({ length: 1, start: () => 0, end: () => end })
	});
	video.currentTime = at;
}

async function mountPlayingHls(): Promise<FakeHlsT> {
	useShowHandlers();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	app = mount(PlayPage, { target });
	const video = getGlobalVideo();
	await until(
		() => video.parentElement?.classList.contains('player-video-slot') === true,
		'the video in its slot'
	);
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => hlsInstances().length > 0, 'the hls engine to attach');
	video.dispatchEvent(new Event('playing'));
	return hlsInstances()[hlsInstances().length - 1];
}

const NETWORK_FATAL = { fatal: true, type: 'networkError', details: 'fragLoadError' };
const HOST_SLOW_FATAL = { fatal: true, type: 'networkError', details: 'fragLoadTimeOut' };

describe('play route — a network failure with buffered media in hand is held', () => {
	it('shows nothing, swaps nothing, and asks the engine to load again after a delay', async () => {
		const hls = await mountPlayingHls();
		bufferedTo(getGlobalVideo(), 300, 10);
		const toastsBefore = toastStore.items.length;
		const streamsBefore = FakeEventSource.instances.length;

		hls.emit(ERROR, NETWORK_FATAL);
		expect(hls.startLoadCalls).toBe(0);
		await new Promise((r) => setTimeout(r, 150));
		expect(toastStore.items.length).toBe(toastsBefore);
		expect(FakeEventSource.instances.length).toBe(streamsBefore);

		await until(() => hls.startLoadCalls === 1, 'the held retry', HOLD_DELAYS_MS[0] + 3000);
		expect(toastStore.items.length).toBe(toastsBefore);
		expect(FakeEventSource.instances.length).toBe(streamsBefore);
	});

	it('holds a host-slow timeout too, without the nudge notice', async () => {
		const hls = await mountPlayingHls();
		bufferedTo(getGlobalVideo(), 300, 10);
		const toastsBefore = toastStore.items.length;

		hls.emit(ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(0);
		await new Promise((r) => setTimeout(r, 150));
		expect(toastStore.items.length).toBe(toastsBefore);
	});

	it('takes the recovery once the buffer is nearly out', async () => {
		const hls = await mountPlayingHls();
		bufferedTo(getGlobalVideo(), 14, 10);
		const streamsBefore = FakeEventSource.instances.length;

		hls.emit(ERROR, NETWORK_FATAL);
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the recovery resolve stream'
		);
	});
});
