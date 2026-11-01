// Acceptance: a captions pick made through the browser's own controls
// — shown when the app's custom controls are turned off — is
// remembered for the show as a pick in the app's menu is, and the
// app's own start of a session is not taken for one.

import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { setLocale } from '../../src/lib/paraglide/runtime';
import { storedCaption } from '../../src/lib/play/caption-choice';
import { playerVideo, playerVideoInSlot } from './player-video';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';

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

const KITSU_ID = '42';
const OTHER_SHOW = '77';

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		thumbnail: null,
		synopsis: null
	}));
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	setLocale('en', { reload: false });
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const start = Date.now();
	while (!predicate()) {
		if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for ${what}`);
		await new Promise((r) => setTimeout(r, 10));
	}
}

/** The session's listing: English (the default), Português and
 *  Español — or, for a session named `*-no-es`, Russian in Español's
 *  place, an episode that lacks a language the show was watched in. */
const LISTING = (session: string) => [
	{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/${session}/sub/0.vtt` },
	{ lang: 'pt-BR', label: 'Português', default: false, url: `${API_BASE}/s/${session}/sub/1.vtt` },
	session.endsWith('-no-es')
		? { lang: 'ru', label: 'Русский', default: false, url: `${API_BASE}/s/${session}/sub/2.vtt` }
		: {
				lang: 'es-419',
				label: 'Español',
				default: false,
				url: `${API_BASE}/s/${session}/sub/2.vtt`
			}
];

function handlers() {
	server.use(
		http.get(`${API_BASE}/api/kitsu/anime/${OTHER_SHOW}`, () =>
			HttpResponse.json(kitsuRef(OTHER_SHOW, 'Another Show', 12))
		),
		http.get(`${API_BASE}/api/kitsu/airing/${OTHER_SHOW}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), use_custom_player_controls: false })
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json(kitsuRef(KITSU_ID, 'The Show', 12))
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({ available: true, episode_count: 12, extra_episodes: [] })
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/s/:session/subtitles`, ({ params }) =>
			HttpResponse.json(LISTING(String(params.session)))
		)
	);
}

async function mountAt(session: string, episode: string, show = KITSU_ID) {
	setParams({ id: show });
	setUrl(`/play/${show}`, { session, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => video.textTracks.length === 3, 'the three tracks attached');
	return video;
}

// In a browser a <track> element owns one TextTrack: its `track`, which
// the video's `textTracks` lists, and whose mode the picker sets. happy-
// dom differs twice, and both would hide what these scenarios assert:
//
// - it builds a new TextTrack on every read of `track`
//   (HTMLTrackElement.js, `get track`), its mode derived from the
//   `default` attribute, so a mode set through `textTracks` is gone on
//   the next read;
// - its TextTrack drops any mode but `showing` and `disabled`
//   (TextTrack.js, `set mode`), so a track set `hidden` — the mode that
//   still loads a track's file — would read as whatever it was before.
//
// For these scenarios each element keeps the first TextTrack it builds,
// as a browser's does, and that TextTrack holds any of the three modes,
// so a picker that hid a track instead of disabling it reads `hidden`.
let trackDescriptor: PropertyDescriptor | undefined;
beforeAll(() => {
	trackDescriptor = Object.getOwnPropertyDescriptor(HTMLTrackElement.prototype, 'track');
	const build = trackDescriptor?.get;
	if (!build) throw new Error('happy-dom has no HTMLTrackElement.track getter to stabilise');
	const owned = new WeakMap<HTMLTrackElement, TextTrack>();
	Object.defineProperty(HTMLTrackElement.prototype, 'track', {
		configurable: true,
		get(this: HTMLTrackElement) {
			let track = owned.get(this);
			if (!track) {
				const built = build.call(this) as TextTrack;
				let mode: TextTrackMode = built.mode;
				Object.defineProperty(built, 'mode', {
					configurable: true,
					get: () => mode,
					set: (next: TextTrackMode) => {
						if (next === 'showing' || next === 'hidden' || next === 'disabled') mode = next;
					}
				});
				track = built;
				owned.set(this, track);
			}
			return track;
		}
	});
});
// The page listens on the video's TextTrackList for the changes the
// browser's own captions menu makes. happy-dom builds a new list on
// every read of `textTracks`, so a listener never hears a change; for
// these scenarios each video keeps one list, an EventTarget over the
// tracks its <track> elements own, as a browser's does.
let listDescriptor: PropertyDescriptor | undefined;
beforeAll(() => {
	listDescriptor = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, 'textTracks');
	const lists = new WeakMap<HTMLMediaElement, EventTarget>();
	Object.defineProperty(HTMLMediaElement.prototype, 'textTracks', {
		configurable: true,
		get(this: HTMLMediaElement) {
			let list = lists.get(this);
			if (!list) {
				const select = this.querySelectorAll.bind(this);
				const tracks = () => Array.from(select('track')).map((el) => el.track);
				list = new Proxy(new EventTarget(), {
					get(target, prop) {
						if (prop === 'length') return tracks().length;
						if (prop === Symbol.iterator) return () => tracks()[Symbol.iterator]();
						if (typeof prop === 'string' && /^\d+$/.test(prop)) return tracks()[Number(prop)];
						const value = Reflect.get(target, prop, target);
						return typeof value === 'function' ? value.bind(target) : value;
					}
				});
				lists.set(this, list);
			}
			return list;
		}
	});
});

afterAll(() => {
	if (trackDescriptor) Object.defineProperty(HTMLTrackElement.prototype, 'track', trackDescriptor);
	if (listDescriptor)
		Object.defineProperty(HTMLMediaElement.prototype, 'textTracks', listDescriptor);
});

/** What the browser's own captions menu does on a pick: shows the
 *  track labelled `label` (or none, for `off`), and the list reports a
 *  change. */
function nativePick(video: HTMLVideoElement, label: string | 'off') {
	for (const t of Array.from(video.textTracks)) t.mode = t.label === label ? 'showing' : 'disabled';
	video.textTracks.dispatchEvent(new Event('change'));
}

/** The label of the one track showing, or null when none is. */
function showing(video: HTMLVideoElement): string | null {
	const on = Array.from(video.textTracks).filter((t) => t.mode === 'showing');
	return on.length === 1 ? on[0].label : on.length === 0 ? null : `${on.length} tracks`;
}

/** Move the mounted page on to another session of the same show. */
async function nextEpisode(video: HTMLVideoElement, session: string, episode: string) {
	setUrl(`/play/${KITSU_ID}`, { session, episode, kind: 'mp4' });
	await until(() => video.src.includes(session), `${session} attached`);
	await until(
		() =>
			video.querySelectorAll('track').length === 3 &&
			Array.from(video.querySelectorAll('track')).every((t) =>
				t.getAttribute('src')?.includes(`/s/${session}/`)
			),
		`${session}'s tracks attached`
	);
}

// Which track a session starts on is the one attached as the default,
// which the browser turns on by itself; every other track starts
// disabled and is never fetched. happy-dom reads the attribute the same
// way, so the scenarios read the mode each track starts in.
// With the app's custom controls turned off the video shows the
// browser's own controls, whose captions menu sets the tracks' modes
// itself; the page hears it as a change on the TextTrackList.
describe("a pick in the browser's own captions menu", () => {
	it("is remembered for the show's next episode", async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
		nativePick(video, 'Español');
		await until(() => storedCaption(KITSU_ID) === 'es-419', 'the pick remembered');
		await nextEpisode(video, 'session-2', '2');
		expect(showing(video)).toBe('Español');
	});

	it('remembers Off', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		nativePick(video, 'off');
		await until(() => storedCaption(KITSU_ID) === 'off', 'Off remembered');
		await nextEpisode(video, 'session-2', '2');
		expect(showing(video)).toBeNull();
	});

	it("does not take the session's own start for a pick", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('Português');
		video.textTracks.dispatchEvent(new Event('change'));
		await new Promise((r) => setTimeout(r, 50));
		expect(storedCaption(KITSU_ID)).toBeUndefined();
	});
});
