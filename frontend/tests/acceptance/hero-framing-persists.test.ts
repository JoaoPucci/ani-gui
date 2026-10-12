// Acceptance: a framing chosen on the home page is the framing the
// detail page opens with. The two pages share nothing but storage, so
// this is the one place the hand-off between them is exercised.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, homeHandlers, kitsuRef } from './home-handlers';

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
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';
import { HERO_PREFS_STORAGE_KEY } from '../../src/lib/hero/framing';
import { m } from '../../src/lib/paraglide/messages';

const KITSU_ID = '7';
const BANNER = 'https://s4.anilist.co/file/anilistcdn/media/anime/banner/7.jpg';
const show = () => ({
	...kitsuRef(KITSU_ID, 'Mushishi', 26),
	status: 'finished',
	poster_image: { large: 'https://media.kitsu.app/anime/7/poster_image/large.jpg' },
	cover_image: {
		tiny: null,
		small: null,
		large: 'https://media.kitsu.app/anime/7/cover_image/large.jpg',
		original: 'https://media.kitsu.app/anime/7/cover_image/original.jpg'
	},
	anilist_banner: BANNER
});

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
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([show()]))
		]),
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () => HttpResponse.json(show())),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 26, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/tmp'))
	);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	localStorage.clear();
});

async function heroImage(selector: string): Promise<HTMLImageElement> {
	await vi.waitFor(() => expect(target.querySelector(selector)).not.toBeNull(), {
		timeout: 3000
	});
	return target.querySelector(selector) as HTMLImageElement;
}

describe('a framing chosen on one page', () => {
	it('is the framing the other page opens with', async () => {
		localStorage.setItem(
			HERO_PREFS_STORAGE_KEY,
			JSON.stringify({ framing: 'pillar', source: 'anilist', controls: true })
		);

		app = mount(HomePage, { target });
		await heroImage('.hero-layer .hero-img');
		const fit = Array.from(target.querySelectorAll('.hero-framing-switch button')).find(
			(b) => b.textContent?.trim() === m.hero_framing_contain()
		) as HTMLButtonElement;
		fit.click();
		await vi.waitFor(() =>
			expect(target.querySelector('.hero-layer')?.classList.contains('framing-contain')).toBe(true)
		);
		unmount(app);
		app = null;

		app = mount(DetailPage, { target });
		await heroImage('section.hero img.hero-img');
		expect(target.querySelector('section.hero')?.classList.contains('framing-contain')).toBe(true);
		const pressed = target.querySelector('.hero-framing-switch button[aria-pressed="true"]');
		expect(pressed?.textContent?.trim()).toBe(m.hero_framing_contain());
	});
});
