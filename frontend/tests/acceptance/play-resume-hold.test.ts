// Acceptance: an episode opened with a point to resume at does not
// show its first frame. The player keeps its loading treatment up —
// the frame is busy, the picture hidden, the controls inert — until
// the resume seek has landed at the point and playback has started,
// then reveals the stream there. An episode with no point opens as it
// always did.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { savePosition } from '../../src/lib/play/watch-position';
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

vi.mock('hls.js', () => {
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = { ERROR: 'hlsError', FRAG_LOADED: 'hlsFragLoaded' };
		static isSupported() {
			return true;
		}
		constructor() {
			FakeHls.instances.push(this);
		}
		loadSource() {}
		attachMedia() {}
		on() {}
		startLoad() {}
		stopLoad() {}
		destroy() {}
	}
	return { default: FakeHls };
});

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { playerVideo, playerVideoInSlot } from './player-video';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	window.localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
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

function useShowHandlers(
	opts: {
		autoSkipOp?: boolean;
		skips?: { skip_type: string; start_time: number; end_time: number }[];
	} = {}
) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), auto_skip_op: opts.autoSkipOp === true })
		),
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
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(opts.skips ?? []))
	);
}

const frame = () => target.querySelector('section.player-frame') as HTMLElement;
const playButton = () =>
	target.querySelector(
		`button[aria-label="${m.play_controls_play_aria_label()}"]`
	) as HTMLButtonElement | null;
const busy = () => frame()?.getAttribute('aria-busy') === 'true';
const inert = (el: Element | null) => el?.closest('[inert]') != null;

/** The page's video, with `play` counted. Starting playback the way
 *  Chromium does fires the element's `play` and `playing`. */
async function openEpisode(
	kind: 'hls' | 'mp4',
	handlers: Parameters<typeof useShowHandlers>[0] = {}
) {
	useShowHandlers(handlers);
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => playButton() !== null, 'the custom controls');
	const video = playerVideo();
	const play = vi.fn(() => {
		video.dispatchEvent(new Event('play'));
		video.dispatchEvent(new Event('playing'));
		return Promise.resolve();
	});
	video.play = play as unknown as HTMLVideoElement['play'];
	Object.defineProperty(video, 'duration', { configurable: true, get: () => 1420 });
	return { video, play };
}

function press(key: string) {
	const ev = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
	document.body.dispatchEvent(ev);
}

describe('play route — a resumed episode opens at its point', () => {
	for (const kind of ['hls', 'mp4'] as const) {
		it(`holds the loading treatment, with inert controls, until the seek lands (${kind})`, async () => {
			savePosition(KITSU_ID, 1, 612.5, 1420);
			const { video, play } = await openEpisode(kind);

			// Opening: busy, the picture hidden, nothing to press.
			await until(() => busy(), 'the frame to hold');
			expect(frame().classList.contains('player-resuming')).toBe(true);
			expect(target.querySelector('.player-spinner')).not.toBeNull();
			expect(inert(playButton())).toBe(true);
			expect(video.autoplay).toBe(false);
			// Neither the shortcut nor a click on the picture starts
			// playback while the resume is on its way.
			press(' ');
			video.dispatchEvent(new MouseEvent('click', { bubbles: true }));
			expect(play).not.toHaveBeenCalled();

			video.currentTime = 0;
			video.dispatchEvent(new Event('loadedmetadata'));
			expect(video.currentTime).toBe(612.5);
			// The seek has not landed: still held.
			await new Promise((r) => setTimeout(r, 20));
			expect(busy()).toBe(true);

			video.dispatchEvent(new Event('seeked'));
			expect(play).toHaveBeenCalledTimes(1);
			await until(() => !busy(), 'the frame to reveal');
			expect(frame().classList.contains('player-resuming')).toBe(false);
			expect(target.querySelector('.player-spinner')).toBeNull();
			expect(inert(target.querySelector('.player-controls'))).toBe(false);
			expect(video.currentTime).toBe(612.5);
		});
	}

	it('an episode with no point opens without holding', async () => {
		const { video } = await openEpisode('hls');
		await new Promise((r) => setTimeout(r, 20));
		expect(busy()).toBe(false);
		expect(frame().classList.contains('player-resuming')).toBe(false);
		expect(inert(playButton())).toBe(false);
		expect(video.autoplay).toBe(true);
	});

	it('a point inside an auto-skipped opening is not skipped from under the hold', async () => {
		// Auto-skip seeks past the opening as soon as the playhead is in
		// it. During the hold that seek would move the playhead off the
		// point, and the hold would never see its seek land; the skip
		// waits for the reveal instead.
		savePosition(KITSU_ID, 1, 612.5, 1420);
		const { video, play } = await openEpisode('hls', {
			autoSkipOp: true,
			skips: [{ skip_type: 'op', start_time: 600, end_time: 690 }]
		});
		await until(() => busy(), 'the frame to hold');
		video.currentTime = 0;
		video.dispatchEvent(new Event('loadedmetadata'));
		video.dispatchEvent(new Event('durationchange'));
		expect(video.currentTime).toBe(612.5);
		video.dispatchEvent(new Event('timeupdate'));
		await new Promise((r) => setTimeout(r, 100));
		expect(video.currentTime).toBe(612.5);
		video.dispatchEvent(new Event('seeked'));
		expect(play).toHaveBeenCalledTimes(1);
		await until(() => !busy(), 'the frame to reveal');
		await until(() => video.currentTime > 690, 'the opening to be skipped after the reveal');
	});
});
