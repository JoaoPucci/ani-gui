// Acceptance: the play page's background warm narrows with resolution
// caching off (the default) and keeps its wide fan-out when the user
// opts in.
//
// Each warm opens its own resolve stream, so the FakeEventSource
// instance count is the observable: the page's own resolve plus
// however many warms the plan issued. Warm streams are never
// completed here — the prefetch concurrency cap (2) then holds the
// wide queue at two in flight, which is all the wide case needs to
// prove the fan-out survived the opt-in.
//
// The stream is an mp4 session — happy-dom has no MediaSource.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

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

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';

type EsHandler = (ev: MessageEvent) => void;
class FakeEventSource {
	static instances: FakeEventSource[] = [];
	url: string;
	listeners: Record<string, EsHandler[]> = {};
	closed = false;
	constructor(url: string) {
		this.url = url;
		FakeEventSource.instances.push(this);
	}
	addEventListener(name: string, handler: EsHandler) {
		(this.listeners[name] ??= []).push(handler);
	}
	close() {
		this.closed = true;
	}
	dispatch(name: string, data?: string) {
		const ev = { data: data ?? '', type: name } as unknown as MessageEvent;
		for (const h of this.listeners[name] ?? []) h(ev);
	}
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(
		`timed out waiting for ${what}\n--- streams ---\n${FakeEventSource.instances.map((i) => i.url).join('\n')}`
	);
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

function useShowHandlers(config: Record<string, unknown>) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(config)),
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
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

async function mountAndResolve() {
	setUrl(`/play/${KITSU_ID}`, { episode: '1' });
	app = mount(PlayPage, { target });
	await until(() => FakeEventSource.instances.length > 0, 'the initial play stream');
	FakeEventSource.instances[0].dispatch(
		'done',
		JSON.stringify({
			id: 'session-1',
			kind: 'mp4',
			has_subtitles: false,
			quality: '1080',
			mode: 'sub'
		})
	);
	// The page carries the session in the URL and gets there by
	// `goto`, which this tier stubs out — so the stub is moved by
	// hand to where the real navigation would have landed.
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
}

describe('play route — background warm width follows the caching setting', () => {
	it('warms only the next episode with resolution caching off', async () => {
		useShowHandlers(appConfig());
		await mountAndResolve();

		// The page resolve plus exactly one warm, for episode 2.
		await until(() => FakeEventSource.instances.length >= 2, 'the narrow warm to fire');
		await new Promise((r) => setTimeout(r, 250));
		expect(FakeEventSource.instances.length).toBe(2);
		expect(FakeEventSource.instances[1].url).toContain('episode=2');
	});

	it('keeps the wide strip fan-out when the user opted into caching', async () => {
		useShowHandlers({ ...appConfig(), cache_resolutions: true });
		await mountAndResolve();

		// The strip's visible episodes queue behind the concurrency
		// cap; more than one warm stream proves the fan-out survived.
		await until(() => FakeEventSource.instances.length >= 3, 'the wide warms to fire');
	});
});
