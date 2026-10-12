// Acceptance (DEV-ONLY experiment): the detail hero renders the framing
// and image source the banner experiment names, master's rendering when
// it names none, and master's blurred poster for a show with no cover.
// Delete with $lib/hero/framing.

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
import { HERO_EXPERIMENT_STORAGE_KEY } from '../../src/lib/hero/framing';

const KITSU_ID = '42';
const COVER = 'https://media.kitsu.app/anime/42/cover_image/large.jpg';
const COVER_WHOLE = 'https://media.kitsu.app/anime/42/cover_image/original.jpg';
const POSTER = 'https://media.kitsu.app/anime/42/poster_image/large.jpg';
const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/42.jpg';

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

function useShow(withCover: boolean) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({
				...kitsuRef(KITSU_ID, 'Ongoing Show', 12),
				status: 'finished',
				poster_image: { large: POSTER },
				cover_image: withCover
					? { tiny: null, small: null, large: COVER, original: COVER_WHOLE }
					: null,
				anilist_banner: withCover ? BANNER : null
			})
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
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/tmp'))
	);
}

function choose(framing: string, source: string) {
	localStorage.setItem(HERO_EXPERIMENT_STORAGE_KEY, JSON.stringify({ framing, source }));
}

async function heroImage(): Promise<HTMLImageElement> {
	await vi.waitFor(() => expect(target.querySelector('section.hero img.hero-img')).not.toBeNull(), {
		timeout: 3000
	});
	return target.querySelector('section.hero img.hero-img') as HTMLImageElement;
}
const proxied = (url: string) => `url=${encodeURIComponent(url)}`;
const framingClasses = () =>
	['framing-contain', 'framing-pillar', 'framing-focus'].filter((f) =>
		target.querySelector('section.hero')?.classList.contains(f)
	);

describe('detail hero framing experiment', () => {
	it('renders master’s framing and Kitsu’s cover when no experiment is chosen', async () => {
		useShow(true);
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(COVER));
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('fills the height with AniList’s banner over a blurred copy under the AniList source', async () => {
		useShow(true);
		choose('pillar', 'anilist');
		app = mount(DetailPage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(
			img.getAttribute('src')
		);
	});

	it('never crops the whole Kitsu upload under the Kitsu source', async () => {
		useShow(true);
		choose('contain', 'kitsu');
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-contain']);
	});

	it('keeps master’s blurred poster for a show with no cover', async () => {
		useShow(false);
		choose('focus', 'anilist');
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(POSTER));
		expect(target.querySelector('section.hero')?.classList.contains('hero-fallback')).toBe(true);
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});
});
