// Acceptance: the player starts a master at the level the quality
// setting names, and its menu lets the viewer choose another.
//
// The policy has its own specs; this drives the page wiring. The
// mocked engine announces a master of three levels the way hls.js
// does once it has parsed one, and the assertions are what the
// viewer gets: the engine set to the highest level before it loads a
// segment, a Quality row in the player's menu naming what plays, the
// levels by height under it, and a choice that reaches the engine at
// once and is what the row names afterwards. A stream of one level
// has nothing to choose and gets no row.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
import { qualityPin } from '../../src/lib/play/arm-quality';

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

// The engine double: the page's hls.js surface, with the level
// setters recorded and an event registry the test emits through. The
// event names are hls.js's own.
vi.mock('hls.js', () => {
	type Handler = (event: string, data: unknown) => void;
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = {
			ERROR: 'hlsError',
			FRAG_LOADED: 'hlsFragLoaded',
			MANIFEST_PARSED: 'hlsManifestParsed',
			FRAG_BUFFERED: 'hlsFragBuffered',
			FRAG_CHANGED: 'hlsFragChanged',
			FRAG_PARSING_INIT_SEGMENT: 'hlsFragParsingInitSegment'
		};
		static isSupported() {
			return true;
		}
		handlers: Record<string, Handler[]> = {};
		levels: unknown[] = [];
		startLevel = -1;
		manual = -1;
		loads: number[] = [];
		nexts: number[] = [];
		starts = 0;
		constructor(readonly config: Record<string, unknown> = {}) {
			FakeHls.instances.push(this);
		}
		get loadLevel() {
			return this.manual;
		}
		set loadLevel(level: number) {
			this.manual = level;
			this.loads.push(level);
		}
		get nextLevel() {
			return this.manual;
		}
		set nextLevel(level: number) {
			this.manual = level;
			this.nexts.push(level);
		}
		get autoLevelEnabled() {
			return this.manual === -1;
		}
		loadSource() {}
		attachMedia() {}
		on(event: string, handler: Handler) {
			(this.handlers[event] ??= []).push(handler);
		}
		startLoad() {
			this.starts += 1;
		}
		stopLoad() {}
		destroy() {}
		emit(event: string, data: unknown) {
			for (const h of this.handlers[event] ?? []) h(event, data);
		}
	}
	return { default: FakeHls };
});

import Hls from 'hls.js';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { playerVideoInSlot } from './player-video';

type FakeHlsT = InstanceType<typeof Hls> & {
	config: Record<string, unknown>;
	starts: number;
	loads: number[];
	nexts: number[];
	startLevel: number;
	emit: (event: string, data: unknown) => void;
};
const hlsInstances = () => (Hls as unknown as { instances: FakeHlsT[] }).instances;
const EVENTS = (Hls as unknown as { Events: Record<string, string> }).Events;

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
const MASTER = [
	{ height: 360, width: 640, bitrate: 800_000 },
	{ height: 720, width: 1280, bitrate: 2_500_000 },
	{ height: 1080, width: 1920, bitrate: 5_000_000 }
];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	hlsInstances().length = 0;
	qualityPin.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	qualityPin.clear();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function useShowHandlers() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

async function mountHls(): Promise<FakeHlsT> {
	useShowHandlers();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => hlsInstances().length > 0, 'the hls engine to attach');
	return hlsInstances()[hlsInstances().length - 1];
}

const label = (height: number) => m.play_quality_height({ height: String(height) });

const tick = () => new Promise((r) => setTimeout(r, 0));

function menuButton(): HTMLButtonElement {
	const button = target.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_controls_menu_aria_label()}"]`
	);
	if (!button) throw new Error('no player menu button');
	return button;
}

function menuItems(): HTMLElement[] {
	return [...target.querySelectorAll<HTMLElement>('#pc-overflow-menu [role^="menuitem"]')];
}

function qualityRow(): HTMLElement | undefined {
	return menuItems().find((el) => el.textContent?.includes(m.play_controls_menu_quality_label()));
}

describe('play route — the quality the setting names, and the menu that changes it', () => {
	it('starts a master at its highest level and lets the viewer pin another', async () => {
		const hls = await mountHls();
		hls.emit(EVENTS.MANIFEST_PARSED, { levels: MASTER });
		// The start level is chosen once the settings have answered.
		await until(() => hls.starts === 1, 'the engine to start');
		expect(hls.startLevel).toBe(2);
		expect(hls.loads).toEqual([2]);
		hls.emit(EVENTS.FRAG_CHANGED, { frag: { type: 'main', level: 2 } });
		await tick();

		menuButton().click();
		await until(() => qualityRow() !== undefined, 'the quality row');
		expect(qualityRow()?.textContent).toContain(
			m.play_quality_auto_playing({ quality: label(1080) })
		);

		qualityRow()?.click();
		await until(() => menuItems().some((el) => el.textContent?.includes(label(720))), 'the levels');
		const radios = menuItems().filter((el) => el.getAttribute('role') === 'menuitemradio');
		// Names only: the checked row's mark is decoration, and which row
		// is checked is asserted through aria-checked below.
		expect(radios.map((el) => el.textContent?.replace('✓', '').trim())).toEqual([
			m.play_quality_auto(),
			label(1080),
			label(720),
			label(360)
		]);
		expect(radios.map((el) => el.getAttribute('aria-checked'))).toEqual([
			'true',
			'false',
			'false',
			'false'
		]);

		radios[2].click();
		expect(hls.nexts).toEqual([1]);
		await until(() => target.querySelector('#pc-overflow-menu') === null, 'the menu to close');

		menuButton().click();
		await until(() => qualityRow() !== undefined, 'the quality row again');
		expect(qualityRow()?.textContent).toContain(label(720));
		expect(qualityRow()?.textContent).not.toContain(m.play_quality_auto());
	});

	it("waits for the settings before choosing the start level, and the setting's level is where it starts", async () => {
		// The settings answer after the master's levels have arrived: the
		// engine is made not to start on its own, and starts once the
		// setting — `worst` here — has chosen its level.
		let release: () => void = () => {};
		const gate = new Promise<void>((r) => (release = r));
		useShowHandlers();
		server.use(
			http.get(`${API_BASE}/api/settings`, async () => {
				await gate;
				return HttpResponse.json({ ...appConfig(), quality: 'worst' });
			})
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
		app = mount(PlayPage, { target });
		await until(() => hlsInstances().length > 0, 'the hls engine to attach');
		const hls = hlsInstances()[hlsInstances().length - 1];
		expect(hls.config.autoStartLoad).toBe(false);
		hls.emit(EVENTS.MANIFEST_PARSED, { levels: MASTER });
		await tick();
		expect(hls.loads).toEqual([]);
		expect(hls.starts).toBe(0);
		release();
		await until(() => hls.starts === 1, 'the engine to start');
		expect(hls.loads).toEqual([0]);
		expect(hls.startLevel).toBe(0);
	});

	it('settings that never answer hold the start back only for a bound, then read as best', async () => {
		// The settings are held through every assertion, so the start is
		// chosen without them; then they are answered, so that the request
		// does not outlive the page.
		let release: () => void = () => {};
		const gate = new Promise<void>((r) => (release = r));
		let answered = false;
		useShowHandlers();
		server.use(
			http.get(`${API_BASE}/api/settings`, async () => {
				await gate;
				answered = true;
				return HttpResponse.json(appConfig());
			})
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
		app = mount(PlayPage, { target });
		await until(() => hlsInstances().length > 0, 'the hls engine to attach');
		const hls = hlsInstances()[hlsInstances().length - 1];
		hls.emit(EVENTS.MANIFEST_PARSED, { levels: MASTER });
		await until(() => hls.starts === 1, 'the engine to start without the settings', 10_000);
		expect(hls.loads).toEqual([2]);
		release();
		await until(() => answered, 'the held settings to be answered');
	});

	it('offers no quality row for a stream of one level', async () => {
		const hls = await mountHls();
		hls.emit(EVENTS.MANIFEST_PARSED, { levels: [MASTER[1]] });
		await until(() => hls.starts === 1, 'the engine to start');
		expect(hls.loads).toEqual([]);
		await tick();
		menuButton().click();
		await until(() => menuItems().length > 0, 'the menu');
		expect(qualityRow()).toBeUndefined();
	});
});
