// Acceptance: a session starts on the subtitle track the viewer most
// likely wants — the language last picked on this show, else the UI
// locale's, else the provider's default — and a show's pick, Off
// included, carries over to its later episodes and survives a reload.

import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { setLocale } from '../../src/lib/paraglide/runtime';
import { m } from '../../src/lib/paraglide/messages';
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
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
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

function ccButton(): HTMLButtonElement {
	const button = document.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_controls_cc_aria_label()}"]`
	);
	if (!button) throw new Error('no captions button');
	return button;
}

/** Open the captions menu and pick the item named `name`. */
async function pick(name: string) {
	if (!document.querySelector('#pc-cc-menu')) {
		ccButton().click();
		await until(() => document.querySelector('#pc-cc-menu') !== null, 'the captions menu');
	}
	const item = Array.from(
		document.querySelectorAll<HTMLButtonElement>('#pc-cc-menu [role="menuitemradio"]')
	).find((b) => b.textContent?.trim().endsWith(name));
	if (!item) throw new Error(`no captions item named ${name}`);
	item.click();
	await tick();
}

async function mountAt(session: string, episode: string, show = KITSU_ID) {
	setParams({ id: show });
	setUrl(`/play/${show}`, { session, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => video.textTracks.length === 3, 'the three tracks attached');
	await until(
		() =>
			document.querySelector(`button[aria-label="${m.play_controls_cc_aria_label()}"]`) !== null,
		'the captions button'
	);
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
afterAll(() => {
	if (trackDescriptor) Object.defineProperty(HTMLTrackElement.prototype, 'track', trackDescriptor);
});

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

async function reload(session: string, episode: string, show = KITSU_ID) {
	if (app) unmount(app);
	app = null;
	return mountAt(session, episode, show);
}

// Which track a session starts on is the one attached as the default,
// which the browser turns on by itself; every other track starts
// disabled and is never fetched. happy-dom reads the attribute the same
// way, so the scenarios read the mode each track starts in.
describe('the track a session starts on', () => {
	it("opens on the track in the user's language", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('Português');
	});

	it("falls back to the provider's default when no track is in the user's language", async () => {
		setLocale('ru', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
	});

	it('keeps Off within the episode', async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		await pick(m.play_controls_cc_off());
		await new Promise((r) => setTimeout(r, 50));
		expect(showing(video)).toBeNull();
	});
});

describe("a show's remembered captions", () => {
	it("starts the show's next episode on the language picked, over the user's locale", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Español');
		await nextEpisode(video, 'session-2', '2');
		expect(showing(video)).toBe('Español');
	});

	it("starts the show's next episode off when Off was picked", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		await pick(m.play_controls_cc_off());
		await nextEpisode(video, 'session-2', '2');
		expect(showing(video)).toBeNull();
	});

	it('survives a reload', async () => {
		handlers();
		await mountAt('session-1', '1');
		await pick('Español');
		const reloaded = await reload('session-2', '2');
		expect(showing(reloaded)).toBe('Español');
	});

	it('leaves another show on the locale', async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		await mountAt('session-1', '1');
		await pick('Español');
		const other = await reload('session-9', '1', OTHER_SHOW);
		expect(showing(other)).toBe('Português');
	});

	// A Continue card opened on a guessed match records nothing under
	// the guessed id (play-origin.ts), and the captions pick is no
	// exception: the show it names may not be the show playing.
	it('remembers nothing for a session opened from a guess', async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		setParams({ id: KITSU_ID });
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4', guess: '1' });
		app = mount(PlayPage, { target });
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(() => playerVideo().textTracks.length === 3, 'the three tracks attached');
		await until(
			() =>
				document.querySelector(`button[aria-label="${m.play_controls_cc_aria_label()}"]`) !== null,
			'the captions button'
		);
		await pick('Español');
		const reloaded = await reload('session-2', '2');
		expect(showing(reloaded)).toBe('Português');
	});

	it('falls back to the locale where an episode lacks the language, and keeps the memory', async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Español');
		await nextEpisode(video, 'session-2-no-es', '2');
		expect(showing(video)).toBe('Português');
		await nextEpisode(video, 'session-3', '3');
		expect(showing(video)).toBe('Español');
	});
});
