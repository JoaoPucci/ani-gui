// Acceptance: episode stills AniList's pacing held back fill in on the
// play page's strip once their retry lands, and nothing else moves: the
// player keeps its video element, source, position, paused state and
// tracks, and the strip's cards stay mounted.
//
// The stream is an mp4 session — happy-dom has no MediaSource.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { playerVideo, playerVideoInSlot } from './player-video';

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
const TICKET = 13;
const still = (n: number) => `https://img1.ak.crunchyroll.com/still-${n}.jpg`;

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

function useShowHandlers(land: Promise<void>, episodeCount = 5) {
	const seen = { episodeReads: 0, readsAfter: 0 };
	let hasLanded = false;
	void land.then(() => (hasLanded = true));
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, episodeCount), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({
				aired: episodeCount,
				next_episode: null,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
			const p = new URL(request.url).searchParams.get('page') ?? '1';
			if (p !== '1') return HttpResponse.json([]);
			seen.episodeReads += 1;
			if (hasLanded) seen.readsAfter += 1;
			const first = !hasLanded;
			const eps = kitsuEpisodes(episodeCount).map((ep) => ({
				...ep,
				thumbnail: first ? null : { original: still(ep.number) }
			}));
			return HttpResponse.json(
				eps,
				first ? { headers: { 'x-ani-gui-refill': String(TICKET) } } : undefined
			);
		}),
		http.post(`${API_BASE}/api/refills/wait`, async () => {
			await land;
			return HttpResponse.json([{ ticket: TICKET, landed: true }]);
		}),
		http.post(`${API_BASE}/api/refills/release`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: episodeCount,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
	return seen;
}

function mountAtEpisode(episode: number) {
	setUrl(`/play/${KITSU_ID}`, {
		session: 'session-1',
		episode: String(episode),
		kind: 'mp4'
	});
	app = mount(PlayPage, { target });
}

const cards = () => Array.from(target.querySelectorAll('li[data-ep-num]'));
const stillImgs = () =>
	Array.from(target.querySelectorAll<HTMLImageElement>('li[data-ep-num] img'));

describe('play route — held-back episode stills', () => {
	it('fill in on the strip without touching the player', async () => {
		let land!: () => void;
		const seen = useShowHandlers(new Promise<void>((r) => (land = r)));
		mountAtEpisode(2);
		await until(() => cards().length === 5 && playerVideoInSlot(), 'the strip and the player');
		expect(stillImgs()).toHaveLength(0);

		const video = playerVideo();
		video.currentTime = 42;
		const src = video.getAttribute('src');
		const paused = video.paused;
		const tracks = Array.from(video.querySelectorAll('track'));
		const trackModes = Array.from(video.textTracks ?? []).map((t) => t.mode);
		const before = cards();

		land();
		await until(() => stillImgs().length === 5, 'the stills to fill in');

		// Fetched once more, not on and on.
		expect(seen.readsAfter).toBe(1);
		expect(playerVideo()).toBe(video);
		expect(video.getAttribute('src')).toBe(src);
		expect(video.currentTime).toBe(42);
		expect(video.paused).toBe(paused);
		expect(Array.from(video.querySelectorAll('track'))).toEqual(tracks);
		Array.from(video.querySelectorAll('track')).forEach((t, i) => expect(t).toBe(tracks[i]));
		expect(Array.from(video.textTracks ?? []).map((t) => t.mode)).toEqual(trackModes);
		const after = cards();
		expect(after).toHaveLength(before.length);
		after.forEach((li, i) => expect(li).toBe(before[i]));
		for (const img of stillImgs()) expect(img.classList.contains('img-arrive')).toBe(true);
	});
});
