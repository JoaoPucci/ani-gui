// Acceptance (DEV-ONLY experiment): the home hero renders the framing
// and image source the banner experiment names, and master's rendering
// when it names none. Delete with $lib/hero/framing.

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
import { HERO_EXPERIMENT_STORAGE_KEY } from '../../src/lib/hero/framing';

const COVER = 'https://media.kitsu.app/anime/1/cover_image/large.jpg';
const COVER_WHOLE = 'https://media.kitsu.app/anime/1/cover_image/original.jpg';
const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/1.jpg';

function featured() {
	return {
		...kitsuRef('1', 'Cowboy Bebop', 26),
		cover_image: { tiny: null, small: null, large: COVER, original: COVER_WHOLE },
		anilist_banner: BANNER
	};
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([featured()]))
		])
	);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

async function heroImage(): Promise<HTMLImageElement> {
	await vi.waitFor(() => expect(target.querySelector('.hero .hero-img')).not.toBeNull(), {
		timeout: 3000
	});
	return target.querySelector('.hero .hero-img') as HTMLImageElement;
}
const proxied = (url: string) => `url=${encodeURIComponent(url)}`;

describe('home hero framing experiment', () => {
	it('renders master’s framing and Kitsu’s cover when no experiment is chosen', async () => {
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER));
		const hero = target.querySelector('section.hero') as HTMLElement;
		expect(hero.classList.contains('framing-contain')).toBe(false);
		expect(hero.classList.contains('framing-focus')).toBe(false);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});

	it('never crops: the whole upload over a blurred copy of itself', async () => {
		localStorage.setItem(
			HERO_EXPERIMENT_STORAGE_KEY,
			JSON.stringify({ framing: 'contain', source: 'kitsu' })
		);
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(target.querySelector('section.hero')?.classList.contains('framing-contain')).toBe(true);
		const backdrop = target.querySelector('.hero-backdrop');
		expect(backdrop?.getAttribute('src')).toBe(img.getAttribute('src'));
	});

	it('fills the height: the whole upload over a blurred copy, cropped at the sides only', async () => {
		localStorage.setItem(
			HERO_EXPERIMENT_STORAGE_KEY,
			JSON.stringify({ framing: 'pillar', source: 'kitsu' })
		);
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(COVER_WHOLE));
		expect(target.querySelector('section.hero')?.classList.contains('framing-pillar')).toBe(true);
		expect(target.querySelector('.hero-backdrop')?.getAttribute('src')).toBe(
			img.getAttribute('src')
		);
	});

	it('takes AniList’s banner first under the AniList source', async () => {
		localStorage.setItem(
			HERO_EXPERIMENT_STORAGE_KEY,
			JSON.stringify({ framing: 'focus', source: 'anilist' })
		);
		app = mount(HomePage, { target });
		const img = await heroImage();
		expect(img.getAttribute('src')).toContain(proxied(BANNER));
		expect(target.querySelector('section.hero')?.classList.contains('framing-focus')).toBe(true);
		expect(target.querySelector('.hero-backdrop')).toBeNull();
	});
});
