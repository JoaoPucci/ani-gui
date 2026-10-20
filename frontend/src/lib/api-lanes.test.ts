// Which backend origin each kind of request goes to.
//
// The backend listens on one loopback origin per lane, because
// Chromium opens at most six connections at a time to one host and
// port and queues the rest: requests that hold a socket for as long as
// a provider or an upstream takes must not share a budget with the
// API's short calls or with the player. The wrappers pick the lane;
// these pin which.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	__resetApiBaseForTests,
	availabilityBatch,
	availabilityWarm,
	checkAvailability,
	downloadStream,
	imageProxyUrl,
	kitsuSearch,
	play,
	playExternal,
	playStream,
	playSyncplay,
	settingsGet
} from './api';
import { probeScope } from './availability/probe-scope';

const API = 'http://127.0.0.1:1';
const LANES = {
	stream: 'http://127.0.0.1:2',
	image: 'http://127.0.0.1:3',
	probe: 'http://127.0.0.1:4',
	resolve: 'http://127.0.0.1:5',
	download: 'http://127.0.0.1:6'
};

type G = {
	window?: { aniGui?: { apiBase?: string; lanes?: Record<string, string> } };
	EventSource?: unknown;
	location?: { pathname: string };
	fetch: typeof fetch;
};
const g = globalThis as unknown as G;

class FakeEventSource {
	static urls: string[] = [];
	constructor(url: string) {
		FakeEventSource.urls.push(url);
	}
	addEventListener() {}
	close() {}
}

let fetched: Array<{ url: string; init?: RequestInit }> = [];

function answering(body: unknown = {}) {
	return vi.fn(async (url: string, init?: RequestInit) => {
		fetched.push({ url, init });
		return new Response(JSON.stringify(body), { status: 200 });
	});
}

beforeEach(() => {
	fetched = [];
	FakeEventSource.urls = [];
	__resetApiBaseForTests(API);
	g.window = { aniGui: { apiBase: API, lanes: { ...LANES } } };
	g.EventSource = FakeEventSource;
	g.fetch = answering({ available: true }) as unknown as typeof fetch;
});

afterEach(() => {
	delete g.window;
	delete g.EventSource;
	delete g.location;
});

describe('lanes', () => {
	it('an availability probe goes to the probe lane', async () => {
		await checkAvailability({ title: 'X', mode: 'sub' });
		expect(fetched[0].url).toBe(`${LANES.probe}/api/availability`);
	});

	it("the cache-only batch and the fire-and-forget warm stay on the API's lane", async () => {
		g.fetch = answering({ cached: {}, playable_episode_counts: {} }) as unknown as typeof fetch;
		await availabilityBatch(['1'], 'sub');
		await availabilityWarm([{ title: 'X', mode: 'sub' }]);
		expect(fetched.map((f) => f.url)).toEqual([
			`${API}/api/availability/batch`,
			`${API}/api/availability/warm`
		]);
	});

	it("page data, search and settings stay on the API's lane", async () => {
		g.fetch = answering([]) as unknown as typeof fetch;
		await kitsuSearch('x');
		await settingsGet();
		expect(fetched.map((f) => f.url)).toEqual([`${API}/api/kitsu/search`, `${API}/api/settings`]);
	});

	it('every play resolution goes to the resolve lane', async () => {
		const args = { title: 'X', episode: '1', mode: 'sub' as const };
		await play(args);
		await playExternal(args);
		await playSyncplay(args);
		void playStream({ ...args, prefetch: true }, () => {});
		await vi.waitFor(() => expect(FakeEventSource.urls).toHaveLength(1));
		expect(fetched.map((f) => f.url)).toEqual([
			`${LANES.resolve}/api/play`,
			`${LANES.resolve}/api/play/external`,
			`${LANES.resolve}/api/play/syncplay`
		]);
		expect(FakeEventSource.urls[0]).toMatch(new RegExp(`^${LANES.resolve}/api/play/stream\\?`));
	});

	it("a download's progress stream goes to the download lane", async () => {
		void downloadStream({ title: 'X', episode: '1', mode: 'sub' }, () => {});
		await vi.waitFor(() => expect(FakeEventSource.urls).toHaveLength(1));
		expect(FakeEventSource.urls[0]).toMatch(
			new RegExp(`^${LANES.download}/api/download/stream\\?`)
		);
	});

	it('a poster goes to the image lane', () => {
		expect(imageProxyUrl('https://media.kitsu.app/p.jpg')).toBe(
			`${LANES.image}/api/image?url=${encodeURIComponent('https://media.kitsu.app/p.jpg')}`
		);
	});

	it("with no lanes announced everything reaches the API's origin", async () => {
		g.window = { aniGui: { apiBase: API } };
		await checkAvailability({ title: 'X', mode: 'sub' });
		await play({ title: 'X', episode: '1', mode: 'sub' });
		expect(fetched.map((f) => f.url)).toEqual([`${API}/api/availability`, `${API}/api/play`]);
		expect(imageProxyUrl('https://a.b/c.jpg')).toMatch(new RegExp(`^${API}/api/image`));
	});
});

describe('checkAvailability cancellation', () => {
	it('carries the signal it is given into the fetch', async () => {
		const ctrl = new AbortController();
		await checkAvailability({ title: 'X', mode: 'sub' }, { signal: ctrl.signal });
		expect(fetched[0].init?.signal).toBe(ctrl.signal);
	});

	it('a probe asked without one is aborted when its page is left', async () => {
		g.location = { pathname: '/search' };
		let seen: AbortSignal | undefined;
		g.fetch = vi.fn((_url: string, init?: RequestInit) => {
			seen = init?.signal ?? undefined;
			return new Promise<Response>((_res, rej) =>
				seen?.addEventListener('abort', () => rej(new DOMException('aborted', 'AbortError')))
			);
		}) as unknown as typeof fetch;
		const probe = checkAvailability({ title: 'X', mode: 'sub' });
		await vi.waitFor(() => expect(seen).toBeDefined());
		g.location = { pathname: '/anime/1' };
		probeScope.settle();
		await expect(probe).rejects.toMatchObject({ name: 'AbortError' });
	});
});
