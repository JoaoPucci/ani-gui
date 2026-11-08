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
import { setLocale } from '../../src/lib/paraglide/runtime';
import { m } from '../../src/lib/paraglide/messages';
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
	// A pick is remembered per show; each scenario starts with none.
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

/** A listing as the provider labels it — Steel Ball Run's first
 *  episode, in part, for a session named `many-*`. */
const MANY: [string, string, boolean][] = [
	['ar', 'Arabic', false],
	['es-419', 'Spanish (- Latin America | SDH)', false],
	['pt-BR', 'Portuguese (- Brazil)', false],
	['en', 'English', true],
	['th', 'Thai (- SDH)', false],
	['th', 'Thai', false],
	['es-419', 'Spanish (- Latin America)', false]
];

const LISTING = (session: string) =>
	session.startsWith('many-')
		? MANY.map(([lang, label, isDefault], i) => ({
				lang,
				label,
				default: isDefault,
				url: `${API_BASE}/s/${session}/sub/${i}.vtt`
			}))
		: [
				{ lang: 'en', label: 'English', default: true, url: `${API_BASE}/s/${session}/sub/0.vtt` },
				{
					lang: 'pt-BR',
					label: 'Português',
					default: false,
					url: `${API_BASE}/s/${session}/sub/1.vtt`
				},
				{
					lang: 'es-419',
					label: 'Español',
					default: false,
					url: `${API_BASE}/s/${session}/sub/2.vtt`
				}
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

async function mountAt(session: string, episode: string, tracks = 3) {
	setUrl(`/play/${KITSU_ID}`, { session, episode, kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	const video = playerVideo();
	await until(() => video.textTracks.length === tracks, 'the tracks attached');
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
		await pick('Portuguese (Brazil)');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'showing'],
			['Español', 'disabled']
		]);
	});

	it('switching tracks moves the one shown and leaves the one before disabled', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Portuguese (Brazil)');
		await pick('Spanish (Latin America)');
		expect(modes(video)).toEqual([
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'showing']
		]);
	});

	it('Off shows no track and leaves every track unloaded', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Portuguese (Brazil)');
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
		await pick('Portuguese (Brazil)');
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

	// A pick is the show's from then on: the next episode's tracks are
	// new elements, and they start on the language picked; a reload of
	// the page does the same (play-captions-start.test.ts covers the
	// rest of what is remembered).
	it('the next episode, and a reload, start on the language picked', async () => {
		handlers();
		const video = await mountAt('session-1', '1');
		await pick('Spanish (Latin America)');

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
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'showing']
		]);

		if (app) unmount(app);
		app = null;
		const reloaded = await mountAt('session-2', '2');
		expect(modes(reloaded)).toEqual([
			['English', 'disabled'],
			['Português', 'disabled'],
			['Español', 'showing']
		]);
	});
});

/** The open menu's entries as they read, in order, with the divider
 *  between the viewer's languages and the rest as `---`. */
function menuReads(): string[] {
	const menu = document.querySelector('#pc-cc-menu');
	if (!menu) throw new Error('the captions menu is not open');
	return Array.from(menu.querySelectorAll('[role="menuitemradio"], [role="separator"]')).map(
		(el) =>
			el.getAttribute('role') === 'separator'
				? '---'
				: (el.textContent ?? '').replace('✓', '').replace(/\s+/g, ' ').trim()
	);
}

async function openMenu() {
	ccButton().click();
	await until(() => document.querySelector('#pc-cc-menu') !== null, 'the captions menu');
	await tick();
}

// A provider lists dozens of languages under labels of its own
// ("Portuguese (- Brazil)", "Thai (- SDH)"). The menu names each from
// its language tag in the app's language, marks the tracks for the
// deaf and hard of hearing instead of spelling it out, puts the
// viewer's languages first and the rest in alphabetical order, and
// stays inside the player however many there are.
describe('the captions menu with many languages', () => {
	it('names each track in the app language, marks SDH, and puts your language first', async () => {
		handlers();
		await mountAt('many-1', '1', MANY.length);
		await openMenu();
		expect(menuReads()).toEqual([
			'Off',
			'English',
			'---',
			'Arabic',
			'Portuguese (Brazil)',
			'Spanish (Latin America)',
			'Spanish (Latin America) SDH',
			'Thai',
			'Thai SDH'
		]);
	});

	it("names them in the app's own language when the app is in Portuguese", async () => {
		setLocale('pt-BR', { reload: false });
		handlers();
		await mountAt('many-1', '1', MANY.length);
		await openMenu();
		expect(menuReads()).toEqual([
			'Desativadas',
			'Português (Brasil)',
			'---',
			'Árabe',
			'Espanhol (América Latina)',
			'Espanhol (América Latina) CC',
			'Inglês',
			'Tailandês',
			'Tailandês CC'
		]);
	});

	it('puts the language last picked on the show among yours', async () => {
		handlers();
		await mountAt('many-1', '1', MANY.length);
		await pick('Thai');
		await openMenu();
		expect(menuReads().slice(0, 5)).toEqual(['Off', 'Thai', 'Thai SDH', 'English', '---']);
	});

	it('fits inside the player and scrolls, rather than running above it', async () => {
		handlers();
		await mountAt('many-1', '1', MANY.length);
		// happy-dom lays nothing out: the player starts 100 px down the
		// window and the captions button 420 px down.
		const frame = document.querySelector<HTMLElement>('.player-frame');
		if (!frame) throw new Error('no player frame');
		frame.getBoundingClientRect = () => new DOMRect(0, 100, 1280, 700);
		ccButton().getBoundingClientRect = () => new DOMRect(1080, 420, 48, 48);
		await openMenu();
		const menu = document.querySelector<HTMLElement>('#pc-cc-menu');
		// 420 − 100, less the gap under the menu and the margin over it:
		// less than half the 700 px player.
		expect(menu?.style.maxBlockSize).toBe('304px');
	});

	it("stops at half the player's height in a tall player", async () => {
		handlers();
		await mountAt('many-1', '1', MANY.length);
		const frame = document.querySelector<HTMLElement>('.player-frame');
		if (!frame) throw new Error('no player frame');
		// A 1000 px player from 100 px down, its button 1000 px down: 884
		// px of room above the button, of which half the player is kept.
		frame.getBoundingClientRect = () => new DOMRect(0, 100, 1280, 1000);
		ccButton().getBoundingClientRect = () => new DOMRect(1080, 1000, 48, 48);
		await openMenu();
		const menu = document.querySelector<HTMLElement>('#pc-cc-menu');
		expect(menu?.style.maxBlockSize).toBe('500px');
	});

	it('stops at the top bar when the page has scrolled the player up under it', async () => {
		handlers();
		await mountAt('many-1', '1', MANY.length);
		const frame = document.querySelector<HTMLElement>('.player-frame');
		if (!frame) throw new Error('no player frame');
		// The player scrolled 200 px above the window; the app's top
		// bar, sticky, ends 84 px down.
		const topbar = document.createElement('header');
		topbar.className = 'topbar';
		topbar.getBoundingClientRect = () => new DOMRect(0, 0, 1280, 84);
		document.body.prepend(topbar);
		try {
			frame.getBoundingClientRect = () => new DOMRect(0, -200, 1280, 700);
			ccButton().getBoundingClientRect = () => new DOMRect(1080, 420, 48, 48);
			await openMenu();
			const menu = document.querySelector<HTMLElement>('#pc-cc-menu');
			// 420 − 84, less the gap under the menu and the margin over it.
			expect(menu?.style.maxBlockSize).toBe('320px');
		} finally {
			topbar.remove();
		}
	});

	it('opens on the selected entry, and arrow keys move through the entries, not the volume', async () => {
		handlers();
		const video = await mountAt('many-1', '1', MANY.length);
		video.volume = 0.5;
		await openMenu();
		const focused = () =>
			(document.activeElement?.textContent ?? '').replace('✓', '').replace(/\s+/g, ' ').trim();
		expect(focused()).toBe('English');
		const press = (key: string) =>
			document.activeElement?.dispatchEvent(
				new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })
			);
		press('ArrowDown');
		expect(focused()).toBe('Arabic');
		press('ArrowUp');
		press('ArrowUp');
		expect(focused()).toBe('Off');
		press('End');
		expect(focused()).toBe('Thai SDH');
		press('Home');
		expect(focused()).toBe('Off');
		expect(video.volume).toBe(0.5);
	});
});
