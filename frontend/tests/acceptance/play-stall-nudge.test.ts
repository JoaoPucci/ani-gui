// Acceptance: a host-slow fatal on a playing HLS stream nudges the
// SAME stream instead of re-resolving.
//
// The pure ladder has its own specs; this drives the page wiring —
// the mocked player engine emits the fatal the way hls.js does, and
// the assertions are what the user experiences: startLoad on the
// stream it already has, one toast naming the slow host, no resolve
// stream (no session swap, no overlay), and the escalation to the
// real recovery only once the burst budget is spent.

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

// The engine double: the page's whole hls.js surface (isSupported,
// construct, loadSource/attachMedia, on, startLoad, destroy) with an
// event registry the test emits through.
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
		loadedSource: string | null = null;
		constructor() {
			FakeHls.instances.push(this);
		}
		loadSource(url: string) {
			this.loadedSource = url;
		}
		attachMedia() {}
		on(event: string, handler: Handler) {
			(this.handlers[event] ??= []).push(handler);
		}
		startLoad() {
			this.startLoadCalls += 1;
		}
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

const HOST_SLOW_FATAL = { fatal: true, type: 'networkError', details: 'fragLoadTimeOut' };

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
	// Minutes into a working stream — the URL has proven itself.
	video.currentTime = 300;
	return hlsInstances()[hlsInstances().length - 1];
}

describe('play route — host-slow fatals nudge the same stream', () => {
	it('startLoad, one toast, no re-resolve', async () => {
		const hls = await mountPlayingHls();
		const toastsBefore = toastStore.items.length;
		const streamsBefore = FakeEventSource.instances.length;

		hls.emit((Hls as unknown as { Events: { ERROR: string } }).Events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(1);

		await until(() => toastStore.items.length > toastsBefore, 'the nudge toast');
		const toast = toastStore.items[toastStore.items.length - 1];
		expect(toast.message).toBe(m.play_stall_toast_nudge());
		// Same stream retried — no session swap, no resolve stream.
		await new Promise((r) => setTimeout(r, 100));
		expect(FakeEventSource.instances.length).toBe(streamsBefore);

		// The burst's later nudges retry without stacking toasts.
		hls.emit((Hls as unknown as { Events: { ERROR: string } }).Events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(2);
		expect(toastStore.items.length).toBe(toastsBefore + 1);
	});

	it('a successful fragment ends the burst — later stalls get fresh nudges', async () => {
		// Three isolated timeouts separated by hours of healthy
		// playback are three bursts of one, not one burst of three: a
		// fragment landing proves the network recovered, so the next
		// stall must start its own budget instead of inheriting a
		// nearly-spent one and escalating into the disruptive
		// re-resolve.
		const hls = await mountPlayingHls();
		const streamsBefore = FakeEventSource.instances.length;
		const events = (Hls as unknown as { Events: { ERROR: string; FRAG_LOADED: string } }).Events;

		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(1);
		// The nudge worked: VIDEO media flows again.
		hls.emit(events.FRAG_LOADED, { frag: { type: 'main' } });

		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(4);
		await new Promise((r) => setTimeout(r, 100));
		expect(FakeEventSource.instances.length).toBe(streamsBefore);
	});

	it('side-rendition fragments do not refill the burst', async () => {
		// Subtitle and audio fragments keep landing while the video
		// rendition times out — they prove nothing about the stall,
		// and resetting on them would keep the burst from ever
		// escalating to the fresh-link recovery.
		const hls = await mountPlayingHls();
		const streamsBefore = FakeEventSource.instances.length;
		const events = (Hls as unknown as { Events: { ERROR: string; FRAG_LOADED: string } }).Events;

		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		hls.emit(events.FRAG_LOADED, { frag: { type: 'subtitle' } });
		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		hls.emit(events.FRAG_LOADED, { frag: { type: 'audio' } });
		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(3);

		hls.emit(events.ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(3);
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the recovery resolve stream'
		);
	});

	it('an exhausted burst escalates to the real recovery', async () => {
		const hls = await mountPlayingHls();
		const streamsBefore = FakeEventSource.instances.length;

		const ERROR = (Hls as unknown as { Events: { ERROR: string } }).Events.ERROR;
		hls.emit(ERROR, HOST_SLOW_FATAL);
		hls.emit(ERROR, HOST_SLOW_FATAL);
		hls.emit(ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(3);

		// The fourth consecutive fatal has outlived the nudges: the
		// evict + fresh-resolve flow takes over.
		hls.emit(ERROR, HOST_SLOW_FATAL);
		expect(hls.startLoadCalls).toBe(3);
		await until(
			() => FakeEventSource.instances.length > streamsBefore,
			'the recovery resolve stream'
		);
	});
});
