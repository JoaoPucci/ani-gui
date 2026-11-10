// Acceptance: from a stream attaching to its first frame, and through
// a stall, the player says the stream is on its way — it never offers
// Play over it.
//
// The page's video is an autoplaying element. Such an element reads
// `paused` until its first fragment is in and fires no `waiting`
// before then, so a player that shows the element's own `paused`
// offers Play over a black frame for as long as the first fragment
// takes: one second on a quick host, fifteen on a slow one, and a
// viewer presses it. Unloading the last stream on an episode switch
// sets `paused` without a `pause` event, so the next one opened under
// the last one's Pause. A stall mid-play showed a frozen picture with
// nothing over it. The play button now reads the viewer's intent, and
// the frame's loading indicator shows through every start and stall,
// not only a resume's.

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
import { readPosition } from '../../src/lib/play/watch-position';
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

const frame = () => target.querySelector('section.player-frame') as HTMLElement;
/** The custom controls' play / pause button, whatever it is labelled. */
const playPause = () => target.querySelector('.pc-buttons-row .pc-btn') as HTMLButtonElement | null;
const label = () => playPause()?.getAttribute('aria-label');
const PLAY = () => m.play_controls_play_aria_label();
const PAUSE = () => m.play_controls_pause_aria_label();
const indicatorOn = () => target.querySelector('.player-spinner.player-spinner-on') !== null;
const busy = () => frame()?.getAttribute('aria-busy') === 'true';
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

describe('play route — a stream on its way is never offered Play', () => {
	it('a clean start reads Pause with the indicator on, until its first frame', async () => {
		const media = await openEpisode();
		expect(playerVideo().paused).toBe(true);
		expect(label()).toBe(PAUSE());
		expect(indicatorOn()).toBe(true);
		expect(busy()).toBe(true);

		media.firstFrame();
		await settle();
		expect(label()).toBe(PAUSE());
		expect(indicatorOn()).toBe(false);
		expect(busy()).toBe(false);
	});

	it('a slow first fragment keeps the start on, whatever the element reports meanwhile', async () => {
		const media = await openEpisode();
		media.fire('loadstart');
		media.fire('loadedmetadata');
		media.fire('progress');
		media.set({ readyState: 1 });
		media.fire('waiting');
		await new Promise((r) => setTimeout(r, 300));
		expect(label()).toBe(PAUSE());
		expect(target.querySelector(`button[aria-label="${PLAY()}"]`)).toBeNull();
		expect(indicatorOn()).toBe(true);
	});

	it('the next stream attaching starts again, though unloading the last fired no pause', async () => {
		const media = await openEpisode();
		media.firstFrame();
		await settle();
		expect(indicatorOn()).toBe(false);

		// The switch's navigation lands on the next session: the page
		// unloads the playing stream, which sets paused silently.
		media.set({ paused: true, readyState: 0 });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'mp4' });
		await until(() => indicatorOn(), 'the next stream to read as starting');
		expect(label()).toBe(PAUSE());
		expect(busy()).toBe(true);

		media.firstFrame();
		await settle();
		expect(indicatorOn()).toBe(false);
	});

	it('a stall mid-play shows the indicator under Pause, until frames play again', async () => {
		const media = await openEpisode();
		media.firstFrame();
		await settle();

		media.set({ readyState: 2 });
		media.fire('waiting');
		await settle();
		expect(indicatorOn()).toBe(true);
		expect(busy()).toBe(true);
		expect(label()).toBe(PAUSE());

		media.set({ readyState: 4 });
		media.fire('playing');
		await settle();
		expect(indicatorOn()).toBe(false);
		expect(busy()).toBe(false);
	});

	it('pressing the button during a start stops it: the autoplay is cancelled and Play is offered', async () => {
		const media = await openEpisode();
		playPause()!.click();
		await settle();
		expect(media.pause).toHaveBeenCalledTimes(1);
		expect(media.play).not.toHaveBeenCalled();
		expect(label()).toBe(PLAY());
		expect(indicatorOn()).toBe(false);

		// The data arriving afterwards starts nothing.
		media.set({ readyState: 4 });
		media.fire('canplaythrough');
		await settle();
		expect(label()).toBe(PLAY());

		// And Play from there starts it again.
		playPause()!.click();
		await settle();
		expect(media.play).toHaveBeenCalledTimes(1);
		expect(label()).toBe(PAUSE());
	});

	it('an autoplay the element refuses reads paused once the data is in', async () => {
		const media = await openEpisode();
		media.set({ readyState: 4 });
		media.fire('canplay');
		media.fire('canplaythrough');
		await settle();
		expect(label()).toBe(PLAY());
		expect(indicatorOn()).toBe(false);
		expect(busy()).toBe(false);
	});

	it('in fullscreen, an idle pointer hides the controls during a start as it does while playing', async () => {
		await openEpisode();
		Object.defineProperty(document, 'fullscreenElement', {
			configurable: true,
			get: () => frame()
		});
		try {
			document.dispatchEvent(new Event('fullscreenchange'));
			await until(
				() => frame().classList.contains('fs-controls-hidden'),
				'the controls to hide once the pointer is idle',
				5000
			);
		} finally {
			delete (document as unknown as { fullscreenElement?: unknown }).fullscreenElement;
			document.dispatchEvent(new Event('fullscreenchange'));
		}
	});

	for (const [what, itemLabel] of [
		['the external player', () => m.play_external_label()],
		['Syncplay', () => m.play_hamburger_syncplay_label()]
	] as const) {
		it(`handing a start to ${what} stops it, so nothing keeps loading behind the other player`, async () => {
			const media = await openEpisode();
			expect(indicatorOn()).toBe(true);
			(
				target.querySelector(
					`button[aria-label="${m.play_more_aria_label()}"]`
				) as HTMLButtonElement
			).click();
			const item = () =>
				Array.from(target.querySelectorAll('button[role="menuitem"]')).find((b) =>
					(b.textContent ?? '').includes(itemLabel())
				) as HTMLButtonElement | undefined;
			await until(() => item() !== undefined, `the ${what} menu item`);
			item()!.click();
			await until(() => media.pause.mock.calls.length > 0, 'the embedded video to pause');
			await settle();
			expect(indicatorOn()).toBe(false);
			expect(busy()).toBe(false);
			expect(label()).toBe(PLAY());
		});
	}

	it('a stream attaching while the error panel is up starts, once the panel gives way to it', async () => {
		await openEpisode();
		const video = playerVideo();
		Object.defineProperty(video, 'error', { value: { code: 3 }, configurable: true });
		video.dispatchEvent(new Event('error'));
		await until(() => target.querySelector('.player-error') !== null, 'the error panel');

		// Back or forward lands on another episode's session while the
		// panel is up: the next stream attaches with no slot on screen.
		Object.defineProperty(video, 'error', { value: null, configurable: true });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'mp4' });
		await until(() => target.querySelector('.player-error') === null, 'the panel to give way');
		await until(() => playPause() !== null, 'the controls back');
		await settle();
		expect(label()).toBe(PAUSE());
		expect(indicatorOn()).toBe(true);
	});

	it("with Chromium's own controls, a start is busy but the frame draws no second indicator", async () => {
		useShowHandlers({ nativeControls: true });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		await until(() => readPosition(KITSU_ID, 1) === 0, 'the stream to attach');
		await until(() => playerVideo().controls, "Chromium's controls");
		await settle();
		expect(busy()).toBe(true);
		expect(indicatorOn()).toBe(false);
	});
});
