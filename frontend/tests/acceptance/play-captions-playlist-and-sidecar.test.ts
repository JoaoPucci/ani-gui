// Acceptance: on a stream whose playlist carries its own subtitle
// renditions and whose session also lists sidecar tracks, the captions
// the viewer sees are the ones the app shows — the session's start, a
// pick from either captions menu — whatever hls.js does with its own
// renditions, and a rendition's cues are there when it is picked.
//
// hls.js is a model of its subtitle handling (tests/hls-subtitle-model.ts),
// since happy-dom runs no Media Source Extensions; the browser is the
// model of Chromium's track selection (browser-track-selection.ts). The
// packaged-app e2e (electron/e2e/captions-playlist-sidecar.spec.ts)
// plays the same shapes through the real hls.js.
//
// The renditions: English (the playlist's default) and Español. The
// sidecar tracks: English, which the UI's language starts on, and
// Français.

import { describe, it, expect, vi, beforeAll, afterAll, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount, tick } from 'svelte';

vi.mock('$app/state', async () => {
	const { page } = await import('./page-state.svelte');
	return {
		get page() {
			return page;
		}
	};
});
vi.mock('$app/navigation', () => ({
	goto: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	beforeNavigate: vi.fn(),
	afterNavigate: vi.fn()
}));

// The engine: the model of hls.js's subtitle handling, plus as much of
// the rest of its surface as the page uses, inert.
vi.mock('hls.js', async () => {
	const { HlsSubtitleModel, MODEL_EVENTS } = await import('../hls-subtitle-model');
	const { addMadeTrack } = await import('./browser-track-selection');
	class FakeHls extends HlsSubtitleModel {
		static instances: FakeHls[] = [];
		static Events = MODEL_EVENTS;
		static isSupported() {
			return true;
		}
		levels: unknown[] = [];
		startLevel = -1;
		nextLevel = -1;
		loadLevel = -1;
		currentLevel = -1;
		autoLevelEnabled = true;
		media: HTMLVideoElement | null = null;
		constructor() {
			let media: HTMLVideoElement | null = null;
			super((kind, label, lang) => {
				if (!media) throw new Error('no media attached');
				return addMadeTrack(media, kind, label, lang);
			});
			this.attachTo = (video: HTMLVideoElement) => {
				media = video;
			};
			FakeHls.instances.push(this);
		}
		attachTo: (video: HTMLVideoElement) => void;
		loadSource() {}
		attachMedia(video: HTMLVideoElement) {
			this.media = video;
			this.attachTo(video);
			this.attach(video);
		}
		startLoad() {}
		stopLoad() {}
		destroy() {}
	}
	return { default: FakeHls };
});

import Hls from 'hls.js';
import { server, API_BASE } from './setup';
import { setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { setLocale } from '../../src/lib/paraglide/runtime';
import { storedCaption } from '../../src/lib/play/caption-choice';
import { playerVideo, playerVideoInSlot } from './player-video';
import { installBrowserTrackSelection } from './browser-track-selection';
import { m } from '../../src/lib/paraglide/messages';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import type { HlsSubtitleModel, Rendition } from '../hls-subtitle-model';

import type {
	AiringStatus,
	AvailabilityResponse,
	Config,
	KitsuAnimeRef,
	KitsuEpisode,
	SessionSubtitle,
	SkipInterval
} from '../../src/lib/api';
import { availabilityAnswer } from '../answers';

const KITSU_ID = '42';
const RENDITIONS: Rendition[] = [
	{ name: 'English', lang: 'en', default: true },
	{ name: 'Español', lang: 'es' }
];

type Engine = HlsSubtitleModel & { media: HTMLVideoElement | null };
const engines = () => (Hls as unknown as { instances: Engine[] }).instances;
const engine = () => {
	const all = engines();
	const e = all[all.length - 1];
	if (!e) throw new Error('no engine made');
	return e;
};

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let browser: { uninstall(): void } | null = null;

beforeAll(() => {
	browser = installBrowserTrackSelection();
});
afterAll(() => {
	browser?.uninstall();
});

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

/** Long enough for the browser's selection, the change it reports, and
 *  hls.js's zero-delay read of the captions after it. */
async function settles() {
	await new Promise((r) => setTimeout(r, 50));
}

/** Holds the session's sidecar listing until {@link releaseListing}. */
let releaseListing: () => void = () => {};

function handlers(customControls = true, { held = false } = {}) {
	const listingIn = held
		? new Promise<void>((resolve) => {
				releaseListing = resolve;
			})
		: Promise.resolve();
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json<Config>({ ...appConfig(), use_custom_player_controls: customControls })
		),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json<KitsuAnimeRef>(kitsuRef(KITSU_ID, 'The Show', 12))
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json<AiringStatus>(null)
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({ available: true, episode_count: 12, extra_episodes: [] })
			)
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json<SkipInterval[]>([])),
		http.get(`${API_BASE}/s/:session/subtitles`, async ({ params }) => {
			await listingIn;
			return HttpResponse.json<SessionSubtitle[]>([
				{
					lang: 'en',
					label: 'English',
					default: true,
					url: `${API_BASE}/s/${params.session}/sub/0.vtt`
				},
				{
					lang: 'fr',
					label: 'Français',
					default: false,
					url: `${API_BASE}/s/${params.session}/sub/1.vtt`
				}
			]);
		})
	);
}

/** Mount the page on an HLS session; the engine is made and attached. */
async function mountHls() {
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => engines().length > 0 && engine().media !== null, 'the engine attached');
	return playerVideo();
}

/** The engine's playlist is in, and its level's subtitle group: it
 *  makes its renditions' tracks and picks its default, English. */
function playlistIn() {
	engine().manifestParsed(RENDITIONS);
	engine().groupArrives();
}

const sidecars = (video: HTMLVideoElement) => video.querySelectorAll('track').length;

/** What shows: each track showing, as rendition or sidecar, by label
 *  as the captions menu reads it. */
function showing(video: HTMLVideoElement): string[] {
	const own = new Set(Array.from(video.querySelectorAll('track'), (el) => el.track));
	return Array.from(video.textTracks)
		.filter((t) => t.mode === 'showing')
		.map((t) => `${own.has(t) ? 'sidecar' : 'playlist'} ${t.label.replaceAll('\u2060', '')}`);
}

/** Open the app's captions menu and pick the item named `name`. */
async function pick(name: string) {
	const button = document.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_controls_cc_aria_label()}"]`
	);
	if (!button) throw new Error('no captions button');
	button.click();
	await until(() => document.querySelector('#pc-cc-menu') !== null, 'the captions menu');
	const item = Array.from(
		document.querySelectorAll<HTMLButtonElement>('#pc-cc-menu [role="menuitemradio"]')
	).find((b) =>
		name === 'Off'
			? b.querySelector('.pc-cc-name') === null
			: b.querySelector('.pc-cc-name')?.textContent?.trim() === name
	);
	if (!item) throw new Error(`no captions item named ${name}`);
	item.click();
	await tick();
	await settles();
}

/** The menu's name for the playlist's English: the two English tracks
 *  read the same, so the menu numbers them in list order. */
function playlistEnglish(video: HTMLVideoElement): string {
	const own = new Set(Array.from(video.querySelectorAll('track'), (el) => el.track));
	const english = Array.from(video.textTracks).filter((t) => t.language === 'en');
	return `English ${english.findIndex((t) => !own.has(t)) + 1}`;
}

/** A pick in Chromium's own captions menu: the chosen track shown, the
 *  one showing switched off. */
function nativePick(video: HTMLVideoElement, which: (t: TextTrack) => boolean) {
	for (const t of Array.from(video.textTracks)) {
		if (which(t)) t.mode = 'showing';
		else if (t.mode === 'showing') t.mode = 'disabled';
	}
}

describe('the session starts on its sidecar track', () => {
	it("after hls.js has picked its playlist's default, and stays alone", async () => {
		handlers(true, { held: true });
		const video = await mountHls();
		playlistIn();
		await settles();
		expect(showing(video)).toEqual(['playlist English']);
		releaseListing();
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		expect(showing(video)).toEqual(['sidecar English']);
		expect(engine().subtitleTrack).toBe(-1);
	});

	it('before hls.js picks its default: its pick comes after, and the start stays alone', async () => {
		handlers();
		const video = await mountHls();
		engine().manifestParsed(RENDITIONS);
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		engine().groupArrives();
		await settles();
		expect(showing(video)).toEqual(['sidecar English']);
		expect(engine().subtitleTrack).toBe(-1);
	});

	it("waits for the engine's playlist before its tracks join", async () => {
		handlers();
		const video = await mountHls();
		await settles();
		await settles();
		expect(sidecars(video)).toBe(0);
		playlistIn();
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
	});

	// hls.js parses the fragment of its default rendition while its
	// track is off, drops the cues, and never fetches it again: the
	// rendition picked later showed nothing.
	it("keeps the cues of the rendition's fragment on its way as it starts", async () => {
		handlers(true, { held: true });
		const video = await mountHls();
		playlistIn();
		const sn = engine().fragmentLoading(0);
		await settles();
		releaseListing();
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		expect(showing(video)).toEqual(['sidecar English']);
		engine().fragmentLands(0, sn, 'A cue from the playlist');
		await settles();
		await pick('Spanish');
		await pick(playlistEnglish(video));
		expect(showing(video)).toEqual(['playlist English']);
		expect(engine().cues[0]).toEqual(['A cue from the playlist']);
	});
});

describe('a pick from the captions menu', () => {
	async function started() {
		handlers();
		const video = await mountHls();
		playlistIn();
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		return video;
	}

	it('of a rendition shows it alone, through the engine', async () => {
		const video = await started();
		await pick('Spanish');
		expect(showing(video)).toEqual(['playlist Español']);
		expect(engine().subtitleTrack).toBe(1);
	});

	it('of a sidecar track while a rendition shows keeps it on', async () => {
		const video = await started();
		await pick('Spanish');
		await pick('French');
		await settles();
		expect(showing(video)).toEqual(['sidecar Français']);
		expect(engine().subtitleTrack).toBe(-1);
	});

	it('of Off leaves nothing showing', async () => {
		const video = await started();
		await pick('Spanish');
		await pick('Off');
		await settles();
		expect(showing(video)).toEqual([]);
	});
});

describe("with the browser's own controls", () => {
	it('a sidecar track picked in its menu stays on and is remembered', async () => {
		handlers(false);
		const video = await mountHls();
		playlistIn();
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		// The viewer moves to the playlist's Español, then to Français.
		nativePick(video, (t) => t.label === 'Español');
		await settles();
		expect(showing(video)).toEqual(['playlist Español']);
		nativePick(video, (t) => t.label === 'Français');
		await settles();
		await settles();
		expect(showing(video)).toEqual(['sidecar Français']);
		expect(engine().subtitleTrack).toBe(-1);
		expect(storedCaption(KITSU_ID)).toBe('fr');
	});

	it("does not take hls.js's own switch for the viewer's pick", async () => {
		handlers(false);
		const video = await mountHls();
		engine().manifestParsed(RENDITIONS);
		await until(() => sidecars(video) === 2, 'the sidecar tracks attached');
		await settles();
		engine().groupArrives();
		await settles();
		expect(showing(video)).toEqual(['sidecar English']);
		expect(storedCaption(KITSU_ID)).toBeUndefined();
	});
});
