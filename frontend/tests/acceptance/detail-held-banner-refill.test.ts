// Acceptance: a show's detail served while AniList's pacing held its
// banner back shows the banner once the retry lands, without the user
// leaving and coming back; a page closed before then fetches nothing
// more and lets the retry go.

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
import { failedHeroImages } from '../../src/lib/hero/framing';

const KITSU_ID = '42';
const POSTER = 'https://media.kitsu.app/anime/42/poster_image/large.jpg';
const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/42.jpg';
const TICKET = 7;

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
	failedHeroImages.clear();
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

/** The show's detail: the first read served without a banner and with
 *  a refill ticket, every later one with the banner. The wait answers
 *  once `land` resolves. */
function useHeldBanner(land: Promise<void>) {
	const seen = { detailReads: 0, released: [] as number[][] };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () => {
			seen.detailReads += 1;
			const first = seen.detailReads === 1;
			const body = {
				...kitsuRef(KITSU_ID, 'Ongoing Show', 12),
				status: 'finished',
				poster_image: { large: POSTER },
				cover_image: first ? null : { tiny: null, small: null, large: BANNER, original: BANNER },
				anilist_banner: first ? null : BANNER
			};
			return HttpResponse.json(
				body,
				first ? { headers: { 'x-ani-gui-refill': String(TICKET) } } : undefined
			);
		}),
		http.post(`${API_BASE}/api/refills/wait`, async () => {
			await land;
			return HttpResponse.json([{ ticket: TICKET, landed: true }]);
		}),
		http.post(`${API_BASE}/api/refills/release`, async ({ request }) => {
			const body = (await request.json()) as { tickets: number[] };
			seen.released.push(body.tickets);
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
			const p = new URL(request.url).searchParams.get('page') ?? '1';
			return HttpResponse.json(p === '1' ? episodes() : []);
		}),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/tmp'))
	);
	return seen;
}

/** The show's twelve episodes, as the page lists them: the banner
 *  refill must not touch them. */
function episodes() {
	return Array.from({ length: 12 }, (_, i) => i + 1).map((n) => ({
		id: `ep-${n}`,
		number: n,
		relative_number: n,
		canonical_title: `Episode ${n}`,
		season_number: 1,
		length: 24,
		synopsis: null,
		airdate: '2019-01-01',
		thumbnail: null
	}));
}

const tiles = () => Array.from(target.querySelectorAll('li[data-ep-num]'));
const proxied = (url: string) => `url=${encodeURIComponent(url)}`;
const heroSrc = () => target.querySelector('section.hero img.hero-img')?.getAttribute('src') ?? '';

describe('a banner held back by AniList’s pacing', () => {
	it('appears once its retry lands, without leaving the page', async () => {
		let land!: () => void;
		const seen = useHeldBanner(new Promise<void>((r) => (land = r)));
		app = mount(DetailPage, { target });
		await vi.waitFor(() => expect(seen.detailReads).toBe(1));
		expect(heroSrc()).not.toContain(proxied(BANNER));
		await vi.waitFor(
			() => expect(target.querySelector('li[data-ep-num="2"]')?.textContent).toContain('Episode 2'),
			{ timeout: 3000 }
		);

		const before = tiles();
		const title = target.querySelector('h1');
		expect(title).not.toBeNull();
		const focused = target.querySelector<HTMLButtonElement>('li[data-ep-num="2"] .ep-tile');
		focused?.focus();
		window.scrollTo(0, 420);
		const scrolled = window.scrollY;
		// The scroll took: the check after the refill compares real
		// positions, not two zeros. The test DOM has no layout, so an
		// element scrolled into view moves nothing here; that call is
		// watched on its own.
		expect(scrolled).toBe(420);
		const intoView = vi.spyOn(Element.prototype, 'scrollIntoView');
		expect(document.activeElement).toBe(focused);

		land();
		await vi.waitFor(() => expect(heroSrc()).toContain(proxied(BANNER)), { timeout: 3000 });
		expect(seen.detailReads).toBe(2);
		// Only the banner changed: the episode tiles, the title, the
		// focused tile and the scroll position are as they were.
		const after = tiles();
		expect(after).toHaveLength(before.length);
		after.forEach((li, i) => expect(li).toBe(before[i]));
		expect(target.querySelector('h1')).toBe(title);
		expect(document.activeElement).toBe(focused);
		expect(window.scrollY).toBe(scrolled);
		expect(intoView).not.toHaveBeenCalled();
		intoView.mockRestore();
	});

	it('is not fetched again for a page closed before it lands, and its retry is let go', async () => {
		let land!: () => void;
		const seen = useHeldBanner(new Promise<void>((r) => (land = r)));
		app = mount(DetailPage, { target });
		await vi.waitFor(() => expect(seen.detailReads).toBe(1));
		unmount(app);
		app = null;
		await vi.waitFor(() => expect(seen.released).toEqual([[TICKET]]));

		land();
		await new Promise((r) => setTimeout(r, 100));
		expect(seen.detailReads).toBe(1);
	});
});
