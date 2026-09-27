// Acceptance: a fragment the engine keeps asking for stops the engine.
//
// The guard has its own specs; this drives the page wiring — the
// mocked engine reports the same fragment loaded again and again, the
// way hls.js does for a fragment it loads but cannot buffer, and the
// assertions are what the user experiences: the engine's loading
// stops once — a later report of the same fragment is not a second
// stop — and the player's error surface names the stream that loads
// but never buffers, in the locale.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
import { FRAGMENT_LOAD_ALLOWANCE } from '../../src/lib/play/fragment-loop-guard';

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

// The engine double: the page's hls.js surface, with stopLoad counted
// and an event registry the test emits through.
vi.mock('hls.js', () => {
	type Handler = (event: string, data: unknown) => void;
	class FakeHls {
		static instances: FakeHls[] = [];
		static Events = { ERROR: 'hlsError', FRAG_LOADED: 'hlsFragLoaded' };
		static isSupported() {
			return true;
		}
		handlers: Record<string, Handler[]> = {};
		stopLoadCalls = 0;
		constructor() {
			FakeHls.instances.push(this);
		}
		loadSource() {}
		attachMedia() {}
		on(event: string, handler: Handler) {
			(this.handlers[event] ??= []).push(handler);
		}
		startLoad() {}
		stopLoad() {
			this.stopLoadCalls += 1;
		}
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
import { getGlobalVideo } from '../../src/lib/play/global-video';

type FakeHlsT = InstanceType<typeof Hls> & {
	stopLoadCalls: number;
	emit: (event: string, data: unknown) => void;
};
const hlsInstances = () => (Hls as unknown as { instances: FakeHlsT[] }).instances;
const FRAG_LOADED = (Hls as unknown as { Events: { FRAG_LOADED: string } }).Events.FRAG_LOADED;

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	hlsInstances().length = 0;
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

function useShowHandlers() {
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
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

async function mountHls(): Promise<FakeHlsT> {
	useShowHandlers();
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
	app = mount(PlayPage, { target });
	const video = getGlobalVideo();
	await until(
		() => video.parentElement?.classList.contains('player-video-slot') === true,
		'the video in its slot'
	);
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => hlsInstances().length > 0, 'the hls engine to attach');
	return hlsInstances()[hlsInstances().length - 1];
}

describe('play route — a fragment loaded past its allowance stops the engine', () => {
	it('stops loading once and names the stream that never buffers', async () => {
		const hls = await mountHls();
		const first = { frag: { type: 'main', level: 0, sn: 1 } };
		for (let i = 0; i < FRAGMENT_LOAD_ALLOWANCE; i++) hls.emit(FRAG_LOADED, first);
		expect(hls.stopLoadCalls).toBe(0);
		expect(target.textContent ?? '').not.toContain(m.play_error_fragment_loop());

		hls.emit(FRAG_LOADED, first);
		expect(hls.stopLoadCalls).toBe(1);
		await until(
			() => (target.textContent ?? '').includes(m.play_error_fragment_loop()),
			'the error surface'
		);
		// A late report of the same fragment is not a second stop.
		hls.emit(FRAG_LOADED, first);
		expect(hls.stopLoadCalls).toBe(1);
	});

	it('does not stop a stream whose renditions each load a fragment twice', async () => {
		// Two seeks within the window: six loads sharing a level and
		// sequence number, two per rendition — past the allowance only
		// if renditions were added up.
		const hls = await mountHls();
		for (let i = 0; i < 2; i++) {
			for (const type of ['main', 'audio', 'subtitle']) {
				hls.emit(FRAG_LOADED, { frag: { type, level: 0, sn: 7 } });
			}
		}
		expect(hls.stopLoadCalls).toBe(0);
	});
});
