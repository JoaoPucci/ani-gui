// Acceptance: an episode handed to the external player or Syncplay is
// not started again in the embedded player by anything but the viewer.
//
// The handoff pauses the embedded video. Before this, every automatic
// path could start it again behind the other player, with sound, or
// keep it loading: the stall recovery (which resolves the episode
// again and attaches a fresh autoplaying stream, after an engine
// failure or a <video> error), the held retry and the stall nudge
// (which ask the engine to load again), a stream that attaches
// meanwhile, and auto-play at the end. Now the handoff stops the
// engine loading and turns the element's autoplay off, those paths
// stand down, and the embedded player starts again only when the
// viewer comes back to it: plays it, picks an episode, or reloads.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { HOLD_DELAYS_MS } from '../../src/lib/play/stall-machine';
import { heldRetries, heldRetriesSettled } from './held-retries';
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

vi.mock('../../src/lib/play/held-retry', async (importOriginal) =>
	(await import('./held-retries')).trackHeldRetries(await importOriginal())
);

vi.mock('hls.js', () => {
	type Handler = (event: string, data: unknown) => void;
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = {
			ERROR: 'hlsError',
			FRAG_LOADED: 'hlsFragLoaded',
			MANIFEST_PARSED: 'hlsManifestParsed'
		};
		static isSupported() {
			return true;
		}
		handlers: Record<string, Handler[]> = {};
		startLoadCalls = 0;
		stopLoadCalls = 0;
		levels: unknown[] = [];
		startLevel = -1;
		loadLevel = -1;
		nextLevel = -1;
		currentLevel = -1;
		autoLevelEnabled = true;
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
		stopLoad() {
			this.stopLoadCalls += 1;
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
import { playerVideo, playerVideoInSlot } from './player-video';

type FakeHlsT = InstanceType<typeof Hls> & {
	startLoadCalls: number;
	stopLoadCalls: number;
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
	heldRetries.length = 0;
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
		// The mount's next-episode prefetch.
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/play/external`, () => new HttpResponse(null, { status: 202 })),
		http.post(`${API_BASE}/api/play/syncplay`, () => new HttpResponse(null, { status: 202 }))
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

async function mountPlayingHls(overrides: () => void = () => {}): Promise<FakeHlsT> {
	useShowHandlers();
	overrides();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => hlsInstances().length > 0, 'the hls engine to attach');
	video.dispatchEvent(new Event('playing'));
	return hlsInstances()[hlsInstances().length - 1];
}

const NETWORK_FATAL = { fatal: true, type: 'networkError', details: 'fragLoadError' };
const HOST_SLOW_FATAL = { fatal: true, type: 'networkError', details: 'fragLoadTimeOut' };
const MANIFEST_PARSED = (Hls as unknown as { Events: { MANIFEST_PARSED: string } }).Events
	.MANIFEST_PARSED;
const LEVELS = { levels: [{ height: 720, width: 1280, bitrate: 2_000_000 }] };

const settle = () => new Promise((r) => setTimeout(r, 30));
const resolves = () =>
	FakeEventSource.instances.filter((i) => i.url.includes('/api/play/stream')).length;

async function handTo(label: string) {
	(
		target.querySelector(`button[aria-label="${m.play_more_aria_label()}"]`) as HTMLButtonElement
	).click();
	const item = () =>
		Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
			(b.textContent ?? '').includes(label)
		) as HTMLButtonElement | undefined;
	await until(() => item() !== undefined, `the ${label} menu item`);
	item()!.click();
	await settle();
}
const handToExternal = () => handTo(m.play_external_label());

describe('play route — an episode handed to another player stays stopped here', () => {
	for (const [what, label] of [
		['the external player', () => m.play_external_label()],
		['Syncplay', () => m.play_hamburger_syncplay_label()]
	] as const) {
		it(`handing it to ${what} stops the engine loading and the element's autoplay`, async () => {
			const hls = await mountPlayingHls();
			await handTo(label());
			expect(hls.stopLoadCalls).toBeGreaterThan(0);
			expect(playerVideo().autoplay).toBe(false);
		});
	}

	it('a held retry does not load again', async () => {
		const hls = await mountPlayingHls();
		await handToExternal();
		bufferedTo(playerVideo(), 300, 10);
		hls.emit(ERROR, NETWORK_FATAL);
		await until(
			heldRetriesSettled,
			'the held retry to fire or be cancelled',
			HOLD_DELAYS_MS[0] + 3000
		);
		expect(hls.startLoadCalls).toBe(0);
	});

	it('a stall nudge does not load again, and says nothing', async () => {
		const hls = await mountPlayingHls();
		await handToExternal();
		bufferedTo(playerVideo(), 10.5, 10);
		const toastsBefore = toastStore.items.length;
		hls.emit(ERROR, HOST_SLOW_FATAL);
		await settle();
		expect(hls.startLoadCalls).toBe(0);
		expect(toastStore.items.length).toBe(toastsBefore);
	});

	it('an engine failure does not resolve the episode again', async () => {
		const hls = await mountPlayingHls();
		await handToExternal();
		const before = resolves();
		bufferedTo(playerVideo(), 10.5, 10);
		hls.emit(ERROR, NETWORK_FATAL);
		hls.emit(ERROR, NETWORK_FATAL);
		hls.emit(ERROR, NETWORK_FATAL);
		await new Promise((r) => setTimeout(r, 300));
		expect(resolves()).toBe(before);
	});

	it("the video element's own network error does not resolve the episode again", async () => {
		await mountPlayingHls();
		await handToExternal();
		const before = resolves();
		const video = playerVideo();
		Object.defineProperty(video, 'error', { value: { code: 2 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await new Promise((r) => setTimeout(r, 300));
		expect(resolves()).toBe(before);
	});

	it('a stream attaching meanwhile neither loads nor plays, until the viewer plays it here', async () => {
		await mountPlayingHls();
		await handToExternal();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '1', kind: 'hls' });
		await until(() => hlsInstances().length > 1, 'the next engine');
		const next = hlsInstances()[hlsInstances().length - 1];
		next.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		expect(next.startLoadCalls).toBe(0);
		expect(playerVideo().autoplay).toBe(false);

		// The viewer plays the embedded video: it loads, and plays on
		// its own again from here on.
		playerVideo().dispatchEvent(new Event('play'));
		await settle();
		expect(next.startLoadCalls).toBe(1);
		expect(playerVideo().autoplay).toBe(true);
	});

	it('playing the embedded video again restarts the loading the handoff stopped', async () => {
		const hls = await mountPlayingHls();
		hls.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		expect(hls.startLoadCalls).toBe(1);
		await handToExternal();
		playerVideo().dispatchEvent(new Event('play'));
		await settle();
		expect(hls.startLoadCalls).toBe(2);
		expect(playerVideo().autoplay).toBe(true);
	});

	it('the end of the episode does not auto-play the next one here', async () => {
		await mountPlayingHls(() =>
			server.use(
				http.get(`${API_BASE}/api/settings`, () =>
					HttpResponse.json({ ...appConfig(), auto_play_next: true })
				)
			)
		);
		await handToExternal();
		playerVideo().dispatchEvent(new Event('ended'));
		await settle();
		// A switch would be on its way: the frame busy with it.
		expect(target.querySelector('section.player-frame.player-busy')).toBeNull();
	});

	it('coming back before the stream has its levels starts it once, where the page starts it', async () => {
		const hls = await mountPlayingHls();
		await handToExternal();
		playerVideo().dispatchEvent(new Event('play'));
		await settle();
		// Nothing was started, so nothing restarts.
		expect(hls.startLoadCalls).toBe(0);
		hls.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		expect(hls.startLoadCalls).toBe(1);
	});

	it('Next while handed off replaces the stream without starting the old one again', async () => {
		const hls = await mountPlayingHls();
		hls.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		await handToExternal();
		const nextButton = () =>
			target.querySelector<HTMLButtonElement>(
				`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
			);
		await until(() => nextButton()?.disabled === false, 'Next enabled');
		nextButton()!.click();
		await settle();
		// The switch is on its way: the old stream stays stopped.
		expect(hls.startLoadCalls).toBe(1);
		expect(playerVideo().autoplay).toBe(false);

		// The switch lands on the next session: that stream starts, and
		// plays on its own.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'hls' });
		await until(() => hlsInstances().length > 1, 'the next engine');
		const next = hlsInstances()[hlsInstances().length - 1];
		next.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		expect(next.startLoadCalls).toBe(1);
		expect(playerVideo().autoplay).toBe(true);
	});

	it('a recovery already evicting when the episode is handed off stands down', async () => {
		let release: () => void = () => {};
		const evicted = new Promise<void>((r) => {
			release = r;
		});
		const hls = await mountPlayingHls(() =>
			server.use(
				http.post(`${API_BASE}/api/play/cache/evict`, async () => {
					await evicted;
					return new HttpResponse(null, { status: 204 });
				})
			)
		);
		bufferedTo(playerVideo(), 10.5, 10);
		const before = resolves();
		hls.emit(ERROR, NETWORK_FATAL);
		await settle();
		await handToExternal();
		release();
		await new Promise((r) => setTimeout(r, 300));
		expect(resolves()).toBe(before);
		// The failed stream's error stands, with its Reload, for when the
		// viewer comes back.
		await until(() => target.querySelector('.player-error-reload') !== null, 'the Reload');
	});

	it('a pick that never landed does not carry into the next handoff', async () => {
		const hls = await mountPlayingHls();
		hls.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		await handToExternal();
		const nextButton = () =>
			target.querySelector<HTMLButtonElement>(
				`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
			);
		await until(() => nextButton()?.disabled === false, 'Next enabled');
		nextButton()!.click();
		await settle();
		// The pick fails: its stream answers with an error.
		const pick = FakeEventSource.instances.filter((e) => e.url.includes('episode=2')).at(-1)!;
		const fail = new MessageEvent('error', { data: JSON.stringify({ kind: 'timeout' }) });
		for (const h of pick.listeners['error'] ?? []) h(fail);
		await until(
			() => target.querySelector('section.player-frame.player-busy') === null,
			'the failed switch to end'
		);
		// The viewer comes back by playing, then hands off again.
		playerVideo().dispatchEvent(new Event('play'));
		await settle();
		await handToExternal();
		expect(playerVideo().autoplay).toBe(false);

		// A stream attaches now, behind the second handoff: the failed
		// pick's flag must not let it play here.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'hls' });
		await until(() => hlsInstances().length > 1, 'the next engine');
		const next = hlsInstances()[hlsInstances().length - 1];
		next.emit(MANIFEST_PARSED, LEVELS);
		await settle();
		expect(next.startLoadCalls).toBe(0);
		expect(playerVideo().autoplay).toBe(false);
	});

	it('a Reload handed off again before its eviction answers stands down too', async () => {
		let release: () => void = () => {};
		const evicted = new Promise<void>((r) => {
			release = r;
		});
		const hls = await mountPlayingHls(() =>
			server.use(
				http.post(`${API_BASE}/api/play/cache/evict`, async () => {
					await evicted;
					return new HttpResponse(null, { status: 204 });
				})
			)
		);
		await handToExternal();
		// The stream fails while handed off: no recovery, its Reload offered.
		bufferedTo(playerVideo(), 10.5, 10);
		hls.emit(ERROR, NETWORK_FATAL);
		hls.emit(ERROR, NETWORK_FATAL);
		hls.emit(ERROR, NETWORK_FATAL);
		await until(() => target.querySelector('.player-error-reload') !== null, 'the Reload');
		const before = resolves();
		// The viewer reloads, then hands off again before the eviction answers.
		(target.querySelector('.player-error-reload') as HTMLButtonElement).click();
		await settle();
		await handToExternal();
		release();
		await new Promise((r) => setTimeout(r, 300));
		expect(resolves()).toBe(before);
	});
});
