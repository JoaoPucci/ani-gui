// Acceptance: the play page owns its video. Leaving the page stops
// and unloads the stream — the engine is destroyed, the element leaves
// the page, nothing pops out into picture-in-picture — and coming back
// to the episode loads it fresh and resumes from where it was left.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { flushSync, mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { readPosition, savePosition } from '../../src/lib/play/watch-position';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));
// Leave hooks the page registers are kept, so a case can run them the
// way the router does before a navigation away.
const leaveHooks = vi.hoisted(() => [] as ((nav: unknown) => void)[]);
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn((hook: (nav: unknown) => void) => {
		leaveHooks.push(hook);
	}),
	afterNavigate: vi.fn()
}));

vi.mock('hls.js', () => {
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = { ERROR: 'hlsError', FRAG_LOADED: 'hlsFragLoaded' };
		static isSupported() {
			return true;
		}
		destroyed = 0;
		sources: string[] = [];
		constructor() {
			FakeHls.instances.push(this);
		}
		loadSource(url: string) {
			this.sources.push(url);
		}
		attachMedia() {}
		on() {}
		startLoad() {}
		stopLoad() {}
		destroy() {
			this.destroyed += 1;
		}
	}
	return { default: FakeHls };
});

import Hls from 'hls.js';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

type FakeHlsT = { destroyed: number; sources: string[] };
const hlsInstances = () => (Hls as unknown as { instances: FakeHlsT[] }).instances;

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
const requestPip = vi.fn(async () => {});

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	hlsInstances().length = 0;
	leaveHooks.length = 0;
	requestPip.mockClear();
	Object.defineProperty(HTMLVideoElement.prototype, 'requestPictureInPicture', {
		configurable: true,
		value: requestPip
	});
	window.localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	Reflect.deleteProperty(HTMLVideoElement.prototype, 'requestPictureInPicture');
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
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

async function mountHls(): Promise<{ hls: FakeHlsT; video: HTMLVideoElement }> {
	useShowHandlers();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	const before = hlsInstances().length;
	app = mount(PlayPage, { target });
	await until(() => target.querySelector('.player-video-slot video') !== null, 'the video');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => hlsInstances().length > before, 'the hls engine to attach');
	const video = target.querySelector('.player-video-slot video') as HTMLVideoElement;
	return { hls: hlsInstances()[hlsInstances().length - 1], video };
}

/** A browser's `load()` puts the element back at zero; happy-dom's
 *  does not, so a case that depends on what is read before the unload
 *  makes the element behave as Chromium's does. */
function resetsOnLoad(video: HTMLVideoElement) {
	const load = video.load.bind(video);
	video.load = () => {
		video.currentTime = 0;
		load();
	};
}

function leave() {
	for (const hook of leaveHooks) {
		hook({ to: { route: { id: '/' }, params: {}, url: new URL('http://localhost/') } });
	}
	unmount(app!);
	app = null;
	flushSync();
}

describe('play route — the page owns its video', () => {
	it('leaving the page destroys the engine and unloads the video, with no picture-in-picture', async () => {
		const { hls, video } = await mountHls();
		video.setAttribute('src', 'blob:http://localhost/engine-source');
		leave();
		expect(requestPip).not.toHaveBeenCalled();
		expect(hls.destroyed).toBe(1);
		expect(video.isConnected).toBe(false);
		expect(video.hasAttribute('src')).toBe(false);
		expect(document.querySelectorAll('video')).toHaveLength(0);
	});

	it('coming back loads the episode fresh and resumes where it was left', async () => {
		const first = await mountHls();
		leave();
		savePosition(KITSU_ID, 1, 612.5, 1420);
		const again = await mountHls();
		expect(again.hls).not.toBe(first.hls);
		expect(again.video).not.toBe(first.video);
		expect(again.hls.sources).toHaveLength(1);
		// A browser knows the stream's length by its metadata; happy-dom
		// reports none, so the case gives the element one.
		Object.defineProperty(again.video, 'duration', { configurable: true, get: () => 1420 });
		again.video.currentTime = 0;
		again.video.dispatchEvent(new Event('loadedmetadata'));
		expect(again.video.currentTime).toBe(612.5);
	});

	it('leaving keeps where the episode was left', async () => {
		// The page writes the position before it unloads the element;
		// unloading first would leave it reading zero.
		const { video } = await mountHls();
		resetsOnLoad(video);
		video.dispatchEvent(new Event('loadedmetadata'));
		video.currentTime = 600;
		leave();
		expect(readPosition(KITSU_ID, 1)).toBe(600);
	});

	it('switching episodes keeps where the previous one was left', async () => {
		// The attach for the next episode writes the previous one's
		// position before it tears the old stream down.
		const { video } = await mountHls();
		resetsOnLoad(video);
		video.dispatchEvent(new Event('loadedmetadata'));
		video.currentTime = 600;
		const before = hlsInstances().length;
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'hls' });
		await until(() => hlsInstances().length > before, 'the next episode to attach');
		expect(readPosition(KITSU_ID, 1)).toBe(600);
		// The next episode is started, not yet anywhere.
		expect(readPosition(KITSU_ID, 2)).toBe(0);
	});

	it('an autoplaying stream whose length arrives late still resumes where it was left', async () => {
		// The element autoplays. When the stream's length is not known by
		// its metadata, playback starts and ticks before it is; the resume
		// waits for the length rather than letting the ticks overwrite
		// the kept point.
		savePosition(KITSU_ID, 1, 612.5, 1420);
		const { video } = await mountHls();
		let length = Number.POSITIVE_INFINITY;
		Object.defineProperty(video, 'duration', { configurable: true, get: () => length });
		video.dispatchEvent(new Event('loadedmetadata'));
		video.dispatchEvent(new Event('playing'));
		video.currentTime = 0.3;
		video.dispatchEvent(new Event('timeupdate'));
		expect(readPosition(KITSU_ID, 1)).toBe(612.5);
		length = 1420;
		video.dispatchEvent(new Event('durationchange'));
		expect(video.currentTime).toBe(612.5);
	});
});
