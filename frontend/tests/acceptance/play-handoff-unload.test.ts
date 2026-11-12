// Acceptance: a stream the element plays from its `src` (mp4, or HLS
// without hls.js) is unloaded when the episode is handed to another
// player, and comes back where it was when the viewer does.
//
// Such a stream has no engine the page can stop: paused, the browser
// may go on buffering it, spending the host's budget on a stream
// nobody watches. The handoff now lets go of it the way a switch lets
// go of a stream — its listeners keep where it was, then it unloads —
// and coming back attaches it again through the same path a resume
// takes: at its kept point, with its subtitles.

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
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

function useShowHandlers(opts: { nativeControls?: boolean } = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({
				...appConfig(),
				use_custom_player_controls: !opts.nativeControls
			})
		),
		http.post(`${API_BASE}/api/play/external`, () => new HttpResponse(null, { status: 202 })),
		http.get(`${API_BASE}/s/:session/subtitles`, () =>
			HttpResponse.json([{ lang: 'en', label: 'English', path: 'subtitles/0', default: true }])
		),
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

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

const src = () => playerVideo().getAttribute('src') ?? '';
const tracks = () => playerVideo().querySelectorAll('track').length;

async function handToExternal() {
	(
		target.querySelector(`button[aria-label="${m.play_more_aria_label()}"]`) as HTMLButtonElement
	).click();
	const item = () =>
		Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
			(b.textContent ?? '').includes(m.play_external_label())
		) as HTMLButtonElement | undefined;
	await until(() => item() !== undefined, 'the external player menu item');
	item()!.click();
	await settle();
}

/** The stream's metadata is in, and it plays at `at`. */
function playingAt(media: ReturnType<typeof controlled>, at: number) {
	const video = playerVideo();
	Object.defineProperty(video, 'duration', { configurable: true, get: () => 1420 });
	video.dispatchEvent(new Event('loadedmetadata'));
	media.firstFrame();
	video.currentTime = at;
	video.dispatchEvent(new Event('timeupdate'));
}

describe('play route — a src stream handed to another player is unloaded, and comes back where it was', () => {
	it('the handoff unloads it and keeps where it was, whatever the unload reports', async () => {
		const media = await openEpisode();
		playingAt(media, 300);
		await until(() => tracks() === 1, 'the subtitle track');
		await handToExternal();
		expect(src()).toBe('');
		expect(readPosition(KITSU_ID, 1)).toBe(300);
		// What the unloaded element reports writes nothing.
		playerVideo().currentTime = 0;
		playerVideo().dispatchEvent(new Event('timeupdate'));
		playerVideo().dispatchEvent(new Event('pause'));
		await settle();
		expect(readPosition(KITSU_ID, 1)).toBe(300);
	});

	it('playing it again attaches it again, at its point, with its subtitles', async () => {
		const media = await openEpisode();
		playingAt(media, 300);
		await until(() => tracks() === 1, 'the subtitle track');
		await handToExternal();
		expect(tracks()).toBe(0);

		playerVideo().dispatchEvent(new Event('play'));
		await until(() => src().includes('session-1'), 'the stream attached again');
		await until(() => tracks() === 1, 'the subtitle track again');
		playerVideo().currentTime = 0;
		playerVideo().dispatchEvent(new Event('loadedmetadata'));
		expect(playerVideo().currentTime).toBe(300);
	});

	it('a handoff before the first frame keeps the kept point, and the return resumes there', async () => {
		savePosition(KITSU_ID, 1, 612.5, 1420);
		useShowHandlers();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => src().includes('session-1'), 'the stream attached');
		controlled(playerVideo());
		await handToExternal();
		expect(src()).toBe('');
		expect(readPosition(KITSU_ID, 1)).toBe(612.5);

		playerVideo().dispatchEvent(new Event('play'));
		await until(() => src().includes('session-1'), 'the stream attached again');
		Object.defineProperty(playerVideo(), 'duration', { configurable: true, get: () => 1420 });
		playerVideo().currentTime = 0;
		playerVideo().dispatchEvent(new Event('loadedmetadata'));
		expect(playerVideo().currentTime).toBe(612.5);
	});

	it('picking the next episode attaches that one, and the handed-off one keeps its point', async () => {
		const media = await openEpisode();
		playingAt(media, 300);
		await handToExternal();
		expect(src()).toBe('');
		const nextButton = () =>
			target.querySelector<HTMLButtonElement>(
				`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
			);
		await until(() => nextButton()?.disabled === false, 'Next enabled');
		nextButton()!.click();
		await settle();
		expect(src()).toBe('');
		// The switch lands on the next session.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'mp4' });
		await until(() => src().includes('session-2'), 'the next stream');
		expect(playerVideo().autoplay).toBe(true);
		expect(readPosition(KITSU_ID, 1)).toBe(300);
	});

	it('Reload from an error panel, after the handoff, attaches the fresh stream', async () => {
		const media = await openEpisode();
		playingAt(media, 300);
		const video = playerVideo();
		Object.defineProperty(video, 'error', { value: { code: 3 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await until(() => target.querySelector('.player-error-reload') !== null, 'the Reload');
		await handToExternal();
		// The error panel stands in for the slot: the element is read
		// directly.
		expect(video.getAttribute('src')).toBeNull();

		const before = FakeEventSource.instances.length;
		(target.querySelector('.player-error-reload') as HTMLButtonElement).click();
		await until(() => FakeEventSource.instances.length > before, 'the fresh resolve');
		FakeEventSource.instances.at(-1)!.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-3',
				media_url: `${API_BASE}/s/session-3/file.mp4`,
				media_kind: 'mp4'
			})
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the switch to navigate');
		setUrl(`/play/${KITSU_ID}`, { session: 'session-3', episode: '1', kind: 'mp4' });
		await until(() => target.querySelector('.player-error') === null, 'the panel to give way');
		await until(() => src().includes('session-3'), 'the fresh stream');
		// It resumes at its point: the resume hold starts it there.
		await until(
			() =>
				target.querySelector('section.player-frame')?.classList.contains('player-resuming') ===
				true,
			'the resume hold'
		);
	});

	for (const [where, at] of [
		['in its first seconds', 10],
		['in its last minute and a half', 1360]
	] as const) {
		it(`a stream handed off ${where} comes back exactly where it was`, async () => {
			// The kept position would read these as the start, or as
			// finished; the return goes back to the playhead itself.
			const media = await openEpisode();
			playingAt(media, at);
			await handToExternal();
			expect(src()).toBe('');

			playerVideo().dispatchEvent(new Event('play'));
			await until(() => src().includes('session-1'), 'the stream attached again');
			playerVideo().currentTime = 0;
			playerVideo().dispatchEvent(new Event('loadedmetadata'));
			expect(playerVideo().currentTime).toBe(at);
		});
	}

	it('the handoff playhead stays with the visit: opening the episode again later goes by its kept position', async () => {
		const media = await openEpisode();
		playingAt(media, 1360);
		await handToExternal();
		// The viewer leaves while handed off, and opens the episode again
		// later: in its last stretch it reads as finished, from its start.
		unmount(app!);
		app = null;
		target.replaceChildren();
		setUrl(`/play/${KITSU_ID}`, { session: 'session-4', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => src().includes('session-4'), 'the stream attached');
		Object.defineProperty(playerVideo(), 'duration', { configurable: true, get: () => 1420 });
		playerVideo().currentTime = 0;
		playerVideo().dispatchEvent(new Event('loadedmetadata'));
		expect(playerVideo().currentTime).toBe(0);
	});

	it('Reload after the handoff resumes exactly where it was handed off', async () => {
		const media = await openEpisode();
		playingAt(media, 1360);
		const video = playerVideo();
		Object.defineProperty(video, 'error', { value: { code: 3 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await until(() => target.querySelector('.player-error-reload') !== null, 'the Reload');
		await handToExternal();
		const before = FakeEventSource.instances.length;
		(target.querySelector('.player-error-reload') as HTMLButtonElement).click();
		await until(() => FakeEventSource.instances.length > before, 'the fresh resolve');
		FakeEventSource.instances.at(-1)!.dispatch(
			'done',
			JSON.stringify({
				session_id: 'session-5',
				media_url: `${API_BASE}/s/session-5/file.mp4`,
				media_kind: 'mp4'
			})
		);
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the switch to navigate');
		setUrl(`/play/${KITSU_ID}`, { session: 'session-5', episode: '1', kind: 'mp4' });
		await until(() => src().includes('session-5'), 'the fresh stream');
		Object.defineProperty(playerVideo(), 'error', { value: null, configurable: true });
		Object.defineProperty(playerVideo(), 'duration', { configurable: true, get: () => 1420 });
		playerVideo().currentTime = 0;
		playerVideo().dispatchEvent(new Event('loadedmetadata'));
		expect(playerVideo().currentTime).toBe(1360);
	});
});
