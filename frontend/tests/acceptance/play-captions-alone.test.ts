// Acceptance: the subtitle track a session starts on is the only one
// that shows, once the browser has run its own track selection — and a
// session that starts on none shows none. What shows is also drawn:
// a track can show, its cues active, with nothing on screen.
//
// The browser here is a model of Chromium's selection
// (browser-track-selection.ts): measured on the shipped Electron, a
// start turned on after its tracks joined the video left a second
// language showing beside it, Arabic over English on a nine-language
// episode; a start already showing as it joined showed alone, and was
// never drawn.

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
import {
	installBrowserTrackSelection,
	paintsCaptions,
	setLoadOrder
} from './browser-track-selection';
import { m } from '../../src/lib/paraglide/messages';
import { tick } from 'svelte';
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

/** Long enough for the browser's selection, a zero-delay timer after
 *  the tracks join, and the change it reports. */
async function browserSettles() {
	await new Promise((r) => setTimeout(r, 50));
}

/** Arabic first, as the provider lists it, then English (its default)
 *  and Português — or, for a session named `*-ru`, one Russian track
 *  that is nobody's default: an episode with no track to start on. */
const LISTING = (session: string) =>
	session.endsWith('-ru')
		? [
				{ lang: 'ru', label: 'Русский', default: false, url: `${API_BASE}/s/${session}/sub/0.vtt` },
				{ lang: 'ar', label: 'العربية', default: false, url: `${API_BASE}/s/${session}/sub/1.vtt` },
				{ lang: 'de', label: 'Deutsch', default: false, url: `${API_BASE}/s/${session}/sub/2.vtt` }
			]
		: [
				{ lang: 'ar', label: 'العربية', default: false, url: `${API_BASE}/s/${session}/sub/0.vtt` },
				{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/${session}/sub/1.vtt` },
				{
					lang: 'pt-BR',
					label: 'Português',
					default: false,
					url: `${API_BASE}/s/${session}/sub/2.vtt`
				}
			];

function handlers(customControls = true) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () =>
			HttpResponse.json({ ...appConfig(), use_custom_player_controls: customControls })
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

async function mountAt(session: string, episode: string) {
	setUrl(`/play/${KITSU_ID}`, { session, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => video.textTracks.length === 3, 'the three tracks attached');
	await browserSettles();
	return video;
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
	await browserSettles();
}

/** The label of the one track showing, null when none is, or how many
 *  show when several do. */
function showing(video: HTMLVideoElement): string | null {
	const on = Array.from(video.textTracks).filter((t) => t.mode === 'showing');
	return on.length === 1 ? on[0].label : on.length === 0 ? null : `${on.length} tracks`;
}

describe('the track a session starts on', () => {
	it('shows alone when it is not the first listed', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
	});

	it("shows alone in the user's language", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('Português');
	});

	it('leaves the next episode with nothing showing when it has no track to start on', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
		await nextEpisode(video, 'session-2-ru', '2');
		expect(showing(video)).toBeNull();
	});

	// With the app's custom controls off, a caption change the app did
	// not make is the viewer's pick in the browser's own menu, and is
	// remembered for the show: a track the browser turned on beside the
	// start would be remembered as one.
	it("is not remembered as the viewer's pick with the browser's own controls", async () => {
		handlers(false);
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
		expect(storedCaption(KITSU_ID)).toBeUndefined();
	});
});

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
	).find((b) => b.textContent?.trim().endsWith(name));
	if (!item) throw new Error(`no captions item named ${name}`);
	item.click();
	await tick();
	await browserSettles();
}

describe('the track that shows is drawn', () => {
	it('when the session starts on it', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
	});

	it("when the session starts on it in the user's language", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('Português');
		expect(paintsCaptions(video)).toBe(true);
	});

	it("when the session starts on it with the browser's own controls", async () => {
		handlers(false);
		const video = await mountAt('session-1', '1');
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
	});

	it('when the next episode starts on it', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await nextEpisode(video, 'session-2', '2');
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
	});

	it("when it is picked from the app's captions menu", async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Arabic');
		expect(showing(video)).toBe('العربية');
		expect(paintsCaptions(video)).toBe(true);
	});
});

// Nothing in the browser orders the start's file arriving against its
// own track selection. Whichever comes first, the start shows alone,
// is drawn, and nothing the browser did is remembered as the viewer's
// pick.
describe.each([
	['the selection runs before the start file arrives', 'selection-first'],
	['the start file arrives before the selection runs', 'load-first']
] as const)('when %s', (_name, order) => {
	beforeEach(() => setLoadOrder(order));
	afterEach(() => setLoadOrder('selection-first'));

	it('the start shows alone and is drawn', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await browserSettles();
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
	});

	it("the start shows alone, is drawn, and is no pick with the browser's own controls", async () => {
		handlers(false);
		const video = await mountAt('session-1', '1');
		await browserSettles();
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
		expect(storedCaption(KITSU_ID)).toBeUndefined();
	});

	it('the next episode starts alone and drawn', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await nextEpisode(video, 'session-2', '2');
		await browserSettles();
		expect(showing(video)).toBe('English');
		expect(paintsCaptions(video)).toBe(true);
	});
});
