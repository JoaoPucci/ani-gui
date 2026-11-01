// Acceptance: episode stills AniList's pacing held back fill in on the
// detail page once their retry lands, without disturbing anything else:
// the scroll position, the focused tile and the tiles themselves stay
// as they were, and only the new images appear — fading in.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
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

import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';
const TICKET = 11;
const still = (n: number) => `https://img1.ak.crunchyroll.com/still-${n}.jpg`;

type GlobalLike = { EventSource?: unknown };
const g = globalThis as unknown as GlobalLike;
class SilentEventSource {
	addEventListener() {}
	close() {}
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	g.EventSource = SilentEventSource;
	localStorage.clear();
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	localStorage.clear();
});

function episodes(withStills: boolean) {
	return [1, 2, 3].map((n) => ({
		id: `ep-${n}`,
		number: n,
		relative_number: n,
		canonical_title: `Episode ${n}`,
		season_number: 1,
		length: 24,
		synopsis: null,
		airdate: '2019-01-01',
		thumbnail: withStills ? { original: still(n) } : null
	}));
}

/** The episode page served first without stills and with a refill
 *  ticket; every later read with them. The wait answers once `land`
 *  resolves. */
function useHeldStills(land: Promise<void>) {
	const seen = { episodeReads: 0, readsAfter: 0 };
	let hasLanded = false;
	void land.then(() => (hasLanded = true));
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Ongoing Show', 3), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
			const p = new URL(request.url).searchParams.get('page') ?? '1';
			if (p !== '1') return HttpResponse.json([]);
			seen.episodeReads += 1;
			if (hasLanded) seen.readsAfter += 1;
			const first = !hasLanded;
			return HttpResponse.json(
				episodes(!first),
				first ? { headers: { 'x-ani-gui-refill': String(TICKET) } } : undefined
			);
		}),
		http.post(`${API_BASE}/api/refills/wait`, async () => {
			await land;
			return HttpResponse.json([{ ticket: TICKET, landed: true }]);
		}),
		http.post(`${API_BASE}/api/refills/release`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 3, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 3,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/tmp'))
	);
	return seen;
}

const tiles = () => Array.from(target.querySelectorAll('li[data-ep-num]'));
const stillImgs = () => Array.from(target.querySelectorAll<HTMLImageElement>('.ep-thumb img'));

describe('episode stills held back by AniList’s pacing', () => {
	it('fill in without moving, refocusing or remounting anything', async () => {
		let land!: () => void;
		const seen = useHeldStills(new Promise<void>((r) => (land = r)));
		app = mount(DetailPage, { target });
		await vi.waitFor(() => expect(tiles()).toHaveLength(3), { timeout: 3000 });
		expect(stillImgs()).toHaveLength(0);

		const before = tiles();
		const focused = target.querySelector<HTMLButtonElement>('li[data-ep-num="2"] .ep-tile');
		focused?.focus();
		window.scrollTo(0, 420);
		const scrolled = window.scrollY;
		expect(document.activeElement).toBe(focused);

		land();
		await vi.waitFor(() => expect(stillImgs()).toHaveLength(3), { timeout: 3000 });

		// Fetched once more, not on and on.
		expect(seen.readsAfter).toBe(1);
		// The same tiles, in the same order: nothing remounted.
		const after = tiles();
		expect(after).toHaveLength(before.length);
		after.forEach((li, i) => expect(li).toBe(before[i]));
		expect(document.activeElement).toBe(focused);
		expect(window.scrollY).toBe(scrolled);
		// Only the new images appear, and they fade in.
		expect(stillImgs().map((img) => decodeURIComponent(img.src))).toEqual(
			[1, 2, 3].map((n) => expect.stringContaining(still(n)))
		);
		for (const img of stillImgs()) expect(img.classList.contains('img-arrive')).toBe(true);
	});
});
