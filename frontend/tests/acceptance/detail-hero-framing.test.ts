// Acceptance: the detail hero fills its height with AniList's banner by
// default — in a production build as much as in a dev one — falling
// back to the whole Kitsu cover and then the blurred poster, and
// renders the framing and image source the stored preferences name.

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
import { HERO_PREFS_STORAGE_KEY, failedHeroImages } from '../../src/lib/hero/framing';
import { m } from '../../src/lib/paraglide/messages';

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
	vi.unstubAllEnvs();
});

function useShow({ cover = true, banner = true }: { cover?: boolean; banner?: boolean } = {}) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({
				...kitsuRef(KITSU_ID, 'Ongoing Show', 12),
				status: 'finished',
				poster_image: { large: POSTER },
				cover_image: cover
					? { tiny: null, small: null, large: COVER, original: COVER_WHOLE }
					: null,
				anilist_banner: banner ? BANNER : null
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

function store(framing: string, source: string, controls = false) {
	localStorage.setItem(HERO_PREFS_STORAGE_KEY, JSON.stringify({ framing, source, controls }));
}

/** A production build: `import.meta.env.DEV` is false. */
function production() {
	vi.stubEnv('DEV', false);
	vi.stubEnv('PROD', true);
}

async function heroImage(): Promise<HTMLImageElement> {
	await vi.waitFor(() => expect(target.querySelector('section.hero img.hero-img')).not.toBeNull(), {
		timeout: 3000
	});
	return target.querySelector('section.hero img.hero-img') as HTMLImageElement;
}
const proxied = (url: string) => `url=${encodeURIComponent(url)}`;
const heroSrc = () => target.querySelector('section.hero img.hero-img')?.getAttribute('src') ?? '';
const framingClasses = () =>
	['framing-contain', 'framing-pillar', 'framing-focus'].filter((f) =>
		target.querySelector('section.hero')?.classList.contains(f)
	);
const switchButton = (label: string) =>
	Array.from(target.querySelectorAll('.hero-framing-switch button')).find(
		(b) => b.textContent?.trim() === label
	) as HTMLButtonElement;
const stored = () => JSON.parse(localStorage.getItem(HERO_PREFS_STORAGE_KEY) ?? 'null');

describe('detail hero framing', () => {
	it('fills the height with AniList’s banner over a blurred copy when nothing is stored', async () => {
		useShow();
		app = mount(DetailPage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(
			img.getAttribute('src')
		);
	});

	it('does so in a production build', async () => {
		useShow();
		production();
		app = mount(DetailPage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')).not.toBeNull();
	});

	it('fills the height with the whole Kitsu cover when AniList has no banner', async () => {
		useShow({ banner: false });
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-pillar']);
	});

	it('renders the top crop of Kitsu’s cover when that is stored', async () => {
		useShow();
		store('top', 'kitsu');
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(COVER));
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('never crops the whole Kitsu upload under the Kitsu source', async () => {
		useShow();
		store('contain', 'kitsu');
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-contain']);
	});

	it('keeps the blurred poster for a show with no cover', async () => {
		useShow({ cover: false, banner: false });
		store('focus', 'anilist');
		app = mount(DetailPage, { target });
		expect((await heroImage()).getAttribute('src')).toContain(proxied(POSTER));
		expect(target.querySelector('section.hero')?.classList.contains('hero-fallback')).toBe(true);
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('falls back from a banner that fails to load, to the cover, the poster, and nothing', async () => {
		// happy-dom never fetches an <img>'s source; the `error` a 404 from
		// the image route would raise is fired here.
		useShow();
		app = mount(DetailPage, { target });
		let img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));

		img.dispatchEvent(new Event('error'));
		await vi.waitFor(() => expect(heroSrc()).toContain(proxied(COVER_WHOLE)));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(heroSrc());

		img = await heroImage();
		img.dispatchEvent(new Event('error'));
		await vi.waitFor(() => expect(heroSrc()).toContain(proxied(COVER)));

		img = await heroImage();
		img.dispatchEvent(new Event('error'));
		await vi.waitFor(() => expect(heroSrc()).toContain(proxied(POSTER)));
		expect(target.querySelector('section.hero')?.classList.contains('hero-fallback')).toBe(true);
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();

		img = await heroImage();
		img.dispatchEvent(new Event('error'));
		await vi.waitFor(() => expect(target.querySelector('section.hero img.hero-img')).toBeNull());
		// The band stays, on the show's accent gradient.
		expect(target.querySelector('section.hero')).not.toBeNull();
	});

	it('shows no framing switch until the diagnostics page turns it on', async () => {
		useShow();
		app = mount(DetailPage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).toBeNull();
	});

	it('shows the framing switch once the diagnostics page has turned it on', async () => {
		useShow();
		store('pillar', 'anilist', true);
		app = mount(DetailPage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).not.toBeNull();
	});

	it('shows the framing switch in a production build once it is turned on', async () => {
		useShow();
		production();
		store('pillar', 'anilist', true);
		app = mount(DetailPage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).not.toBeNull();
	});

	it('keeps the switch inside the hero band, where nothing else is fixed', async () => {
		// The page's enter animation leaves a transform on its root, so a
		// `position: fixed` child would be anchored to the page, not the
		// window; inside the band there is nothing to be anchored against.
		useShow();
		store('pillar', 'anilist', true);
		app = mount(DetailPage, { target });
		await heroImage();
		expect(target.querySelector('section.hero .hero-framing-switch')).not.toBeNull();
	});

	it('applies a framing chosen in the switch at once, without a reload, and keeps it', async () => {
		useShow();
		store('pillar', 'anilist', true);
		const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
		app = mount(DetailPage, { target });
		await heroImage();
		switchButton(m.hero_framing_focus()).click();
		await vi.waitFor(() => expect(framingClasses()).toEqual(['framing-focus']));
		expect(target.querySelector('.hero-backdrop')).toBeNull();
		expect(reload).not.toHaveBeenCalled();
		expect(stored()).toEqual({ framing: 'focus', source: 'anilist', controls: true });
		reload.mockRestore();
	});
});
