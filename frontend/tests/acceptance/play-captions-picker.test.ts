// Acceptance: the captions picker shows the track picked and leaves
// every other sidecar track unloaded.
//
// A session's sidecar tracks are attached as <track> elements on the
// page's video (play-sidecar-tracks.test.ts). The picker in the custom
// controls lists them with an "Off" item; picking one shows it, and
// every track not shown is disabled, so the browser never fetches it.
// A listing can carry every language a provider offers — 33 for one
// episode — so a picker that hid the others instead loaded them all
// on the first pick.

import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
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
	const start = Date.now();
	while (!predicate()) {
		if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for ${what}`);
		await new Promise((r) => setTimeout(r, 10));
	}
}

const LISTING = (session: string) => [
	{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/${session}/sub/0.vtt` },
	{ lang: 'pt-BR', label: 'Português', default: false, url: `${API_BASE}/s/${session}/sub/1.vtt` },
	{ lang: 'es-419', label: 'Español', default: false, url: `${API_BASE}/s/${session}/sub/2.vtt` }
];

function handlers() {
	server.use(
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

/** Each attached track's label and mode, in the video's order. */
function modes(video: HTMLVideoElement): Array<[string, TextTrackMode]> {
	return Array.from(video.textTracks).map((t) => [t.label, t.mode]);
}

function ccButton(): HTMLButtonElement {
	const button = document.querySelector<HTMLButtonElement>('button[aria-label="Captions"]');
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

async function mountAt(session: string, episode: string) {
	setUrl(`/play/${KITSU_ID}`, { session, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => video.textTracks.length === 3, 'the three tracks attached');
	await until(
		() => document.querySelector('button[aria-label="Captions"]') !== null,
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

// The browser loads a sidecar track's file only while the track's mode
// is `showing` or `hidden`; a `disabled` track is never fetched (HTML,
// "sourcing out-of-band text tracks"). happy-dom fetches no track file
// at all, so a request count would pass whatever the page did: these
// scenarios assert the mode, which is what decides the fetch in a
// browser.
describe('the captions picker', () => {
	it('starts on the default track with every other track unloaded', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		expect(modes(video)).toEqual([
			['English', 'showing'],
			['Português', 'disabled'],
			['Español', 'disabled']
		]);
	});

	it('picking a track shows it alone, and the others stay unloaded', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Português');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'showing'],
			['Español', 'disabled']
		]);
	});

	it('switching tracks moves the one shown and leaves the one before disabled', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Português');
		await pick('Español');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'showing']
		]);
	});

	it('Off shows no track and leaves every track unloaded', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Português');
		await pick('Off');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'disabled']
		]);
	});

	// hls.js keeps a `metadata` text track on the video for the stream's
	// ID3 cues, hidden so its cues load. It is not a caption: the picker
	// lists it nowhere and must leave its mode alone, a pick and Off
	// alike.
	it('a pick and Off leave a metadata track as it was', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		const id3 = document.createElement('track');
		id3.setAttribute('kind', 'metadata');
		id3.setAttribute('label', 'id3');
		video.appendChild(id3);
		id3.track.mode = 'hidden';
		expect(id3.track.mode).toBe('hidden');
		await pick('Português');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'showing'],
			['Español', 'disabled'],
			['id3', 'hidden']
		]);
		await pick('Off');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'disabled'],
			['id3', 'hidden']
		]);
		const listed = Array.from(document.querySelectorAll('#pc-cc-menu [role="menuitemradio"]')).map(
			(b) => b.textContent?.trim()
		);
		expect(listed.some((text) => text?.includes('id3'))).toBe(false);
	});

	// A pick belongs to the session it was made in, as it always has:
	// the next episode's tracks are new elements, and the browser starts
	// them on the provider's default. Reloading the page does the same.
	it('the next episode, and a reload, start again on the default track', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Español');

		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'mp4' });
		await until(() => video.src.includes('session-2'), 'the second session attached');
		await until(
			() =>
				Array.from(video.querySelectorAll('track')).every((t) =>
					t.getAttribute('src')?.includes('session-2')
				) && video.querySelectorAll('track').length === 3,
			"the second session's tracks attached"
		);
		expect(modes(video)).toEqual([
			['English', 'showing'],
			['Português', 'disabled'],
			['Español', 'disabled']
		]);

		if (app) unmount(app);
		app = null;
		const reloaded = await mountAt('session-2', '2');
		expect(modes(reloaded)).toEqual([
			['English', 'showing'],
			['Português', 'disabled'],
			['Español', 'disabled']
		]);
	});
});
