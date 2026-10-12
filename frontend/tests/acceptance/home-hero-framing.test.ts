// Acceptance: the home hero fills its height with AniList's banner by
// default — in a production build as much as in a dev one — falling
// back to the whole Kitsu cover and then the blurred poster, and
// renders the framing and image source the stored preferences name.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';

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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { HERO_PREFS_STORAGE_KEY } from '../../src/lib/hero/framing';
import { m } from '../../src/lib/paraglide/messages';

const COVER = 'https://media.kitsu.app/anime/1/cover_image/large.jpg';
const COVER_WHOLE = 'https://media.kitsu.app/anime/1/cover_image/original.jpg';
const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/1.jpg';

function featured(banner: string | null = BANNER) {
	return {
		...kitsuRef('1', 'Cowboy Bebop', 26),
		cover_image: { tiny: null, small: null, large: COVER, original: COVER_WHOLE },
		anilist_banner: banner
	};
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
	serve(featured());
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
	vi.unstubAllEnvs();
});

function serve(show: ReturnType<typeof featured> | Record<string, unknown>) {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([show]))
		])
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
	await vi.waitFor(() => expect(target.querySelector('.hero .hero-img')).not.toBeNull(), {
		timeout: 3000
	});
	return target.querySelector('.hero .hero-img') as HTMLImageElement;
}
const proxied = (url: string) => `url=${encodeURIComponent(url)}`;
const layer = () => target.querySelector('.hero-layer') as HTMLElement;
const switchButton = (label: string) =>
	Array.from(target.querySelectorAll('.hero-framing-switch button')).find(
		(b) => b.textContent?.trim() === label
	) as HTMLButtonElement;
const stored = () => JSON.parse(localStorage.getItem(HERO_PREFS_STORAGE_KEY) ?? 'null');
const framingClasses = () =>
	['framing-contain', 'framing-pillar', 'framing-focus'].filter((f) =>
		layer().classList.contains(f)
	);

describe('home hero framing', () => {
	it('fills the height with AniList’s banner over a blurred copy when nothing is stored', async () => {
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(
			img.getAttribute('src')
		);
	});

	it('does so in a production build', async () => {
		production();
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')).not.toBeNull();
	});

	it('fills the height with the whole Kitsu cover when AniList has no banner', async () => {
		serve(featured(null));
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-pillar']);
	});

	it('renders the top crop of Kitsu’s cover when that is stored', async () => {
		store('top', 'kitsu');
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER));
		expect(framingClasses()).toEqual([]);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('never crops: the whole upload over a blurred copy of itself', async () => {
		store('contain', 'kitsu');
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-contain']);
		const backdrop = target.querySelector('.hero-backdrop');
		expect(backdrop?.getAttribute('src')).toBe(img.getAttribute('src'));
	});

	it('fills the height with the whole Kitsu upload under the Kitsu source', async () => {
		store('pillar', 'kitsu');
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(framingClasses()).toEqual(['framing-pillar']);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(
			img.getAttribute('src')
		);
	});

	it('leaves a show with no cover on the blurred poster under every framing', async () => {
		serve({
			...kitsuRef('2', 'Trigun', 26),
			poster_image: { large: 'https://media.kitsu.app/anime/2/poster/large.jpg' },
			cover_image: null
		});
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.classList.contains('fallback')).toBe(true);
		// No framing class anywhere in the hero: its scrim stays the poster's.
		for (const f of ['framing-contain', 'framing-pillar', 'framing-focus']) {
			expect(target.querySelector(`section.hero .${f}, section.hero.${f}`)).toBeNull();
		}
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('takes AniList’s banner first under the centre crop too', async () => {
		store('focus', 'anilist');
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(framingClasses()).toEqual(['framing-focus']);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('shows no framing switch until the diagnostics page turns it on', async () => {
		app = mount(HomePage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).toBeNull();
	});

	it('shows the framing switch once the diagnostics page has turned it on', async () => {
		store('pillar', 'anilist', true);
		app = mount(HomePage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).not.toBeNull();
	});

	it('shows the framing switch in a production build once it is turned on', async () => {
		production();
		store('pillar', 'anilist', true);
		app = mount(HomePage, { target });
		await heroImage();
		expect(target.querySelector('.hero-framing-switch')).not.toBeNull();
	});

	it('keeps the switch inside the hero band, where nothing else is fixed', async () => {
		store('pillar', 'anilist', true);
		app = mount(HomePage, { target });
		await heroImage();
		expect(target.querySelector('section.hero .hero-framing-switch')).not.toBeNull();
	});

	it('applies a framing chosen in the switch at once, without a reload, and keeps it', async () => {
		store('pillar', 'anilist', true);
		const reload = vi.spyOn(window.location, 'reload').mockImplementation(() => {});
		app = mount(HomePage, { target });
		await heroImage();
		switchButton(m.hero_framing_contain()).click();
		await vi.waitFor(() => expect(framingClasses()).toEqual(['framing-contain']));
		expect(reload).not.toHaveBeenCalled();
		expect(stored()).toEqual({ framing: 'contain', source: 'anilist', controls: true });
		expect(switchButton(m.hero_framing_contain()).getAttribute('aria-pressed')).toBe('true');
		reload.mockRestore();
	});

	it('applies a source chosen in the switch at once, and says what is on screen', async () => {
		store('pillar', 'anilist', true);
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		switchButton(m.hero_framing_source_kitsu()).click();
		await vi.waitFor(() =>
			expect(
				(target.querySelector('.hero .hero-img') as HTMLImageElement).getAttribute('src')
			).toContain(proxied(COVER_WHOLE))
		);
		expect(target.querySelector('.hero-framing-switch')?.textContent).toContain(
			m.hero_framing_origin_kitsu()
		);
		expect(stored()).toEqual({ framing: 'pillar', source: 'kitsu', controls: true });
	});
});
