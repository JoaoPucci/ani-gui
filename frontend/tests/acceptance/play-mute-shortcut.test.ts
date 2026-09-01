// Acceptance: `m` / `M` mutes and unmutes the player from the page.
//
// The decision itself is a pure function with its own unit tests; this
// drives a real keydown through the mounted play page and checks what
// the user gets — the singleton <video> flips its muted flag, the
// volume pill opens as feedback, and the mute button relabels.
//
// The stream is an mp4 session: that kind goes straight to the
// element's `src`, where an HLS one would ask hls.js for a
// MediaSource happy-dom does not have and land on the error overlay
// instead of the player. The shortcut does not care which it is.
//
// happy-dom's media element does not fire `volumechange` when `muted`
// changes (a TODO in its source), so after each keypress the test
// fires the event the browser would; the page's `isMuted` mirror and
// therefore the button label hang off that listener.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
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
	setUrl(`/play/${KITSU_ID}`, { episode: '1' });
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

/** The keydown as the page sees it: dispatched on <body>, so the
 *  target is neither a field nor a button, bubbling up to the
 *  window listener. Cancelable so the page's preventDefault shows. */
function press(key: string, init: KeyboardEventInit = {}) {
	const ev = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
	document.body.dispatchEvent(ev);
	return ev;
}

/** What the browser does on its own after `muted` changes. */
function volumeChanged(video: HTMLVideoElement) {
	video.dispatchEvent(new Event('volumechange'));
}

const controls = () => target.querySelector('.player-controls');
const buttonLabelled = (label: string) =>
	target.querySelector(`button[aria-label="${label}"]`) as HTMLButtonElement | null;

describe('play route — m / M toggles mute', () => {
	it('mutes the video, opens the volume pill and relabels the button; M unmutes', async () => {
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
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		app = mount(PlayPage, { target });

		await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({
				id: 'session-1',
				kind: 'mp4',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		// The page carries the session in the URL and gets there by
		// `goto`, which this tier stubs out — so the stub is moved by
		// hand to where the real navigation would have landed.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });

		// The session opens the frame, which pulls the singleton video
		// in and renders the custom controls with their mute button.
		const video = getGlobalVideo();
		await until(
			() =>
				video.parentElement?.classList.contains('player-video-slot') === true &&
				buttonLabelled(m.play_controls_mute_aria_label()) !== null,
			'the video in its slot and the mute button'
		);
		video.muted = false;
		expect(controls()?.classList.contains('volume-revealed')).toBe(false);

		const mute = press('m');
		// The element flips on the keydown itself; the DOM feedback
		// lands on Svelte's next flush.
		expect(video.muted).toBe(true);
		expect(mute.defaultPrevented).toBe(true);
		await until(
			() => controls()?.classList.contains('volume-revealed') === true,
			'the volume pill to open'
		);
		volumeChanged(video);
		await until(
			() => buttonLabelled(m.play_controls_unmute_aria_label()) !== null,
			'the button to offer Unmute'
		);

		press('M');
		expect(video.muted).toBe(false);
		volumeChanged(video);
		await until(
			() => buttonLabelled(m.play_controls_mute_aria_label()) !== null,
			'the button to offer Mute again'
		);
	});

	it('leaves the video alone on auto-repeat, with a modifier, and while typing', async () => {
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
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		app = mount(PlayPage, { target });

		await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
		FakeEventSource.instances[0].dispatch(
			'done',
			JSON.stringify({
				id: 'session-1',
				kind: 'mp4',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		);
		// The page carries the session in the URL and gets there by
		// `goto`, which this tier stubs out — so the stub is moved by
		// hand to where the real navigation would have landed.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		const video = getGlobalVideo();
		await until(
			() => video.parentElement?.classList.contains('player-video-slot') === true,
			'the video in its slot'
		);
		video.muted = false;

		// A held key auto-repeats keydown; only the first press counts.
		const held = press('m', { repeat: true });
		expect(video.muted).toBe(false);
		expect(held.defaultPrevented).toBe(false);

		// Ctrl+M belongs to the browser.
		press('m', { ctrlKey: true });
		expect(video.muted).toBe(false);

		// Typing an `m` into a field must reach the field.
		const field = document.createElement('input');
		target.appendChild(field);
		field.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'm', bubbles: true, cancelable: true })
		);
		expect(video.muted).toBe(false);
	});
});
