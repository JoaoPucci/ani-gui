// Acceptance: the start veil — what the player frame shows from a
// stream attaching to its first frame, and in a stall.
//
// A start shows the episode's still, blurred and dimmed, under a
// loader and a caption (the "still" look), or the same still under a
// title card (the "card" look, picked on the Diagnostics page); the
// controls and the Skip button stay away until the first frame. A
// stall shows a compact loader over the frozen picture. Screen readers
// hear the start and the stall through a live region.

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
import { readPosition, savePosition } from '../../src/lib/play/watch-position';
import { m } from '../../src/lib/paraglide/messages';

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
		thumbnail: { original: `https://media.kitsu.app/episodes/thumbnails/${i + 1}/original.jpg` },
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

function useShowHandlers(opts: { nativeControls?: boolean; skips?: unknown[] } = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({
				...appConfig(),
				use_custom_player_controls: !opts.nativeControls
			})
		),
		http.post(`${API_BASE}/api/play/external`, () => new HttpResponse(null, { status: 202 })),
		http.post(`${API_BASE}/api/play/syncplay`, () => new HttpResponse(null, { status: 202 })),
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(opts.skips ?? [])),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

const frame = () => target.querySelector('section.player-frame') as HTMLElement;
/** The custom controls' play / pause button, whatever it is labelled. */
const playPause = () => target.querySelector('.pc-buttons-row .pc-btn') as HTMLButtonElement | null;
const settle = () => new Promise((r) => setTimeout(r, 20));

/** The page's video with its media state under the scenario's hand:
 *  `paused` and `readyState` as the scenario sets them, `pause()`
 *  counted, and events fired as Chromium fires them. */
function controlled(video: HTMLVideoElement) {
	let paused = true;
	let readyState = 0;
	Object.defineProperty(video, 'paused', { configurable: true, get: () => paused });
	Object.defineProperty(video, 'readyState', { configurable: true, get: () => readyState });
	const pause = vi.fn(() => {
		const was = paused;
		paused = true;
		if (!was) video.dispatchEvent(new Event('pause'));
	});
	const play = vi.fn(() => {
		paused = false;
		video.dispatchEvent(new Event('play'));
		return Promise.resolve();
	});
	video.pause = pause;
	video.play = play as unknown as HTMLVideoElement['play'];
	return {
		pause,
		play,
		/** The first frame plays: autoplay sets paused false, then `play`, `playing`. */
		firstFrame() {
			paused = false;
			readyState = 4;
			video.dispatchEvent(new Event('play'));
			video.dispatchEvent(new Event('playing'));
		},
		fire(name: string) {
			video.dispatchEvent(new Event(name));
		},
		set(next: { paused?: boolean; readyState?: number }) {
			paused = next.paused ?? paused;
			readyState = next.readyState ?? readyState;
		}
	};
}

async function openEpisode(sessionId = 'session-1', episode = '1') {
	useShowHandlers();
	return openMounted(sessionId, episode);
}

async function openMounted(sessionId: string, episode: string) {
	setUrl(`/play/${KITSU_ID}`, { session: sessionId, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => playPause() !== null, 'the custom controls');
	const media = controlled(playerVideo());
	// The source has attached once it marked the episode started.
	await until(() => readPosition(KITSU_ID, 1) === 0, 'the stream to attach');
	await settle();
	return media;
}

const veil = () => target.querySelector('.start-veil') as HTMLElement | null;
const mode = () => frame()?.getAttribute('data-start-veil');
const announced = () => target.querySelector('[role="status"] .vh')?.textContent ?? '';
const controlsInert = () => target.querySelector('.player-controls')?.closest('[inert]') != null;

describe('play route — the start veil', () => {
	it('a start shows the episode still under a loader and its caption, with the controls away', async () => {
		const media = await openEpisode();
		expect(mode()).toBe('start');
		expect(veil()?.getAttribute('data-look')).toBe('still');
		expect(veil()?.querySelector('.backdrop img')?.getAttribute('src')).toContain('original.jpg');
		expect(veil()?.querySelector('.strip')).not.toBeNull();
		expect(veil()?.textContent).toContain(m.playstart_starting({ n: '1' }));
		expect(frame().classList.contains('player-starting')).toBe(true);
		expect(controlsInert()).toBe(true);
		expect(announced()).toBe(m.playstart_loading_aria());

		media.firstFrame();
		await until(() => mode() === 'off', 'the veil to lift at the first frame');
		expect(frame().classList.contains('player-starting')).toBe(false);
		expect(controlsInert()).toBe(false);
		expect(announced()).toBe('');
	});

	it('a stall shows the compact loader over the picture, with no backdrop', async () => {
		const media = await openEpisode();
		media.firstFrame();
		await until(() => mode() === 'off', 'the start to end');
		media.set({ readyState: 2 });
		media.fire('waiting');
		await until(() => mode() === 'stall', 'the stall');
		await until(() => veil()?.getAttribute('data-mode') === 'stall', 'the stall veil');
		expect(veil()?.querySelector('.backdrop')).toBeNull();
		expect(veil()?.querySelector('.pill .strip')).not.toBeNull();
		expect(frame().classList.contains('player-starting')).toBe(false);
		expect(announced()).toBe(m.playstart_buffering());
	});

	it('the title card shows the numeral, the episode and the show, once picked', async () => {
		window.localStorage.setItem('ani-gui:player-start-look', 'card');
		await openEpisode();
		expect(veil()?.getAttribute('data-look')).toBe('card');
		expect(veil()?.querySelector('.numeral')?.textContent).toBe('01');
		expect(veil()?.querySelector('.card-title')?.textContent).toBe('Episode 1');
		expect(veil()?.querySelector('.card-show')?.textContent).toBe(TITLE);
		expect(veil()?.querySelector('.card-status .strip')).not.toBeNull();
	});

	it('a resume says where it lands', async () => {
		window.localStorage.setItem('ani-gui:player-start-look', 'card');
		savePosition(KITSU_ID, 1, 612.5, 1420);
		useShowHandlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => mode() === 'start', 'the resume to hold');
		expect(veil()?.textContent).toContain(m.playstart_resuming({ time: '10:12' }));
	});

	it('the Skip button waits for the first frame', async () => {
		useShowHandlers({ skips: [{ skip_type: 'op', start_time: 0, end_time: 90 }] });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => readPosition(KITSU_ID, 1) === 0, 'the stream to attach');
		const media = controlled(playerVideo());
		const video = playerVideo();
		Object.defineProperty(video, 'duration', { configurable: true, get: () => 1420 });
		video.dispatchEvent(new Event('loadedmetadata'));
		video.dispatchEvent(new Event('durationchange'));
		await new Promise((r) => setTimeout(r, 200));
		expect(target.querySelector('.player-skip-btn')).toBeNull();

		media.firstFrame();
		video.dispatchEvent(new Event('timeupdate'));
		await until(() => target.querySelector('.player-skip-btn') !== null, 'the Skip button');
	});

	it("with Chromium's own controls a start is theirs, but a resume hold still shows the veil", async () => {
		useShowHandlers({ nativeControls: true });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => readPosition(KITSU_ID, 1) === 0, 'the stream to attach');
		await until(() => playerVideo().controls, "Chromium's controls");
		await settle();
		expect(mode()).toBe('off');
		unmount(app);
		app = null;
		target.replaceChildren();

		savePosition(KITSU_ID, 1, 612.5, 1420);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => mode() === 'start', 'the resume hold to show the veil');
	});
});
