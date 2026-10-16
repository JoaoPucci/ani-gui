// Acceptance: the stale-stream recovery evicts the cached resolution
// of the version remembered for the show, and names none when nothing
// is remembered.
//
// The backend keys its resolution cache by the version. A recovery
// that dropped another version's row than the one the failing session
// was resolved from would leave the stale row in place, and the retry
// would hit it again. Harness as in the recovery spec: an mp4 session,
// the video error raised the way Chromium reports a network failure
// mid-play (MediaError code 2 plus an `error` event).

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
vi.mock('$lib/account/push-watched', () => ({
	syncWatchedToTrackers: vi.fn(async () => {})
}));

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { applyChoice } from '../../src/lib/versions/version-choice';
import { playerVideo, playerVideoInSlot } from './player-video';

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
}
type GlobalLike = { EventSource?: typeof FakeEventSource };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	FakeEventSource.instances.length = 0;
	g.EventSource = FakeEventSource;
	window.localStorage.clear();
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

type Posted = Record<string, unknown>;

/** Stubs every request the page makes, and records each eviction. */
function useHandlers(evicted: Posted[]) {
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
		http.post(`${API_BASE}/api/play/cache/evict`, async ({ request }) => {
			evicted.push((await request.json()) as Posted);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

/** Mount the page on a playing mp4 session and fail its stream the
 *  way Chromium reports a network error mid-play. */
async function failThePlayingStream() {
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	// The recovery needs the show detail; the title renders once it lands.
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	const video = playerVideo();
	video.currentTime = 300;
	Object.defineProperty(video, 'error', { value: { code: 2 }, configurable: true });
	video.dispatchEvent(new Event('error'));
}

describe('play route — the stale-stream recovery evicts the remembered version', () => {
	it('names the version remembered for the show on the eviction', async () => {
		applyChoice(KITSU_ID, 'standard');
		const evicted: Posted[] = [];
		useHandlers(evicted);
		await failThePlayingStream();

		await until(() => evicted.length > 0, 'the recovery to evict the stale row');
		expect(evicted[0].episode).toBe('1');
		expect(evicted[0].version).toBe('standard');
	});

	it('names no version on the eviction when nothing is remembered', async () => {
		const evicted: Posted[] = [];
		useHandlers(evicted);
		await failThePlayingStream();

		await until(() => evicted.length > 0, 'the recovery to evict the stale row');
		expect(evicted[0].episode).toBe('1');
		expect('version' in evicted[0]).toBe(false);
	});
});
