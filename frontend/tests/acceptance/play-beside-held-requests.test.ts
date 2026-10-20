// Acceptance: a play starts while everything else the renderer has
// open is still waiting.
//
// Chromium opens at most six connections at a time to one host and
// port and queues the rest, so a request that holds a socket for as
// long as a provider or an upstream takes — an availability probe, a
// play resolution warming the next episode, a download's progress
// stream, a cold poster — can starve whatever shares its origin. On
// 2026-10-17, right after an availability re-key, probes queued for
// tens of seconds and the player's segment fetches were cancelled at
// their ten-second timeout.
//
// The backend now listens on one origin per lane, and the renderer
// sends each kind of request to its own. Happy-dom has no connection
// pool, so what this pins is the renderer's half: with probes,
// resolutions and downloads all held open on their lanes, the play
// page's media request goes out at once, on the stream lane, its page
// data comes back on the API's, and the probes never exceed their cap.
// The packaged app's e2e proves the pool half against real Chromium.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import {
	__resetApiBaseForTests,
	checkAvailability,
	downloadStream,
	playStream
} from '../../src/lib/api';
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
const LANES = {
	stream: 'http://127.0.0.1:31338',
	image: 'http://127.0.0.1:31339',
	probe: 'http://127.0.0.1:31340',
	resolve: 'http://127.0.0.1:31341',
	download: 'http://127.0.0.1:31342'
};

/** Progress streams that never finish — a resolution against a slow
 *  provider, a download minutes long. */
class HeldEventSource {
	static open: HeldEventSource[] = [];
	url: string;
	constructor(url: string) {
		this.url = url;
		HeldEventSource.open.push(this);
	}
	addEventListener() {}
	close() {
		HeldEventSource.open = HeldEventSource.open.filter((e) => e !== this);
	}
}

type Bridge = { apiBase: string; lanes?: Record<string, string> };
const bridge = () => (window as unknown as { aniGui: Bridge }).aniGui;
const g = globalThis as unknown as { EventSource?: unknown };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let held: AbortController;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	bridge().lanes = { ...LANES };
	g.EventSource = HeldEventSource;
	HeldEventSource.open = [];
	held = new AbortController();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	held.abort();
	if (app) unmount(app);
	app = null;
	target.remove();
	delete bridge().lanes;
	delete g.EventSource;
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const start = Date.now();
	while (!predicate()) {
		if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for ${what}`);
		await new Promise((r) => setTimeout(r, 10));
	}
}

describe('a play beside held requests', () => {
	it('reaches the stream lane at once while probes, resolutions and downloads wait', async () => {
		let probesInFlight = 0;
		let mostProbesInFlight = 0;
		const never = () => new Promise<never>(() => {});
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json(kitsuRef(KITSU_ID, 'The Show', 12))
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			// A provider that never answers: every probe holds.
			http.post(`${LANES.probe}/api/availability`, async () => {
				probesInFlight += 1;
				mostProbesInFlight = Math.max(mostProbesInFlight, probesInFlight);
				return never();
			}),
			http.get(`${LANES.stream}/s/session-1/subtitles`, () => HttpResponse.json([]))
		);

		// What the surfaces the user just left — or the ones beside the
		// player — have open: a dozen background probes, a few warming
		// resolutions, several downloads.
		for (let i = 0; i < 12; i++) {
			void checkAvailability(
				{ title: `Show ${i}`, mode: 'sub', kitsu_id: `${100 + i}`, background: true },
				{ signal: held.signal }
			).catch(() => {});
		}
		for (let i = 0; i < 6; i++) {
			void playStream(
				{ title: `Show ${i}`, episode: '2', mode: 'sub', prefetch: true },
				() => {},
				held.signal
			).catch(() => {});
			void downloadStream(
				{ title: `Show ${i}`, episode: '1', mode: 'sub' },
				() => {},
				held.signal
			).catch(() => {});
		}

		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });

		await until(() => playerVideoInSlot(), 'the video in its slot');
		const video = playerVideo();
		await until(() => video.src.length > 0, 'the media request');
		expect(video.src).toBe(`${LANES.stream}/s/session-1/file.mp4`);
		// Its page data came back on the API's lane meanwhile.
		await until(() => target.textContent?.includes('The Show') === true, 'the show title');

		expect(HeldEventSource.open.filter((e) => e.url.startsWith(`${LANES.resolve}/`))).toHaveLength(
			6
		);
		expect(HeldEventSource.open.filter((e) => e.url.startsWith(`${LANES.download}/`))).toHaveLength(
			6
		);
		// The probes never took every connection their lane has, and
		// the background ones left room for the page's own.
		expect(mostProbesInFlight).toBeLessThanOrEqual(4);
		expect(probesInFlight).toBeGreaterThan(0);
	});
});
