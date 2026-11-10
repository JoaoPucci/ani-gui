// Acceptance: Home's hero takes the first three trending shows that
// have art of their own — an AniList banner or a Kitsu cover — in
// trending order, and the trending row below it starts with the shows
// the hero passed over, so no show leaves Home. With fewer than three
// shows with art, the hero fills its rotation with blurred posters. A
// show the trending list repeats is shown once, in its first place.

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
import { failedHeroImages } from '../../src/lib/hero/framing';
import { m } from '../../src/lib/paraglide/messages';

const poster = (id: string) => ({ large: `https://media.kitsu.app/anime/${id}/poster/large.jpg` });

/** A show with only a poster: the hero would have to blur it. */
function bare(id: string, title: string) {
	return { ...kitsuRef(id, title, 12), poster_image: poster(id), cover_image: null };
}

function withCover(id: string, title: string) {
	return {
		...bare(id, title),
		cover_image: { large: `https://media.kitsu.app/anime/${id}/cover_image/large.jpg` }
	};
}

function withBanner(id: string, title: string) {
	return {
		...bare(id, title),
		anilist_banner: `https://s4.anilist.co/file/anilistcdn/media/anime/banner/${id}.jpg`
	};
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	failedHeroImages.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

function serve(trending: unknown[]) {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json(trending))
		])
	);
}

const heroTitle = () => target.querySelector('.hero .hero-title')?.textContent?.trim() ?? null;

const pagerTitles = () =>
	Array.from(target.querySelectorAll('.hero-pager-dot')).map((b) => b.getAttribute('aria-label'));

const dotLabels = (titles: string[]) =>
	titles.map((title) => m.home_hero_pager_dot_label({ title }));

/** The cards of the trending row, by title, in order. */
function trendingRow(): string[] {
	const strip = Array.from(target.querySelectorAll('section.strip')).find((s) =>
		s.querySelector('.eyebrow-key')?.textContent?.includes(m.home_strip_trending_eyebrow())
	);
	return Array.from(strip?.querySelectorAll('.card-title') ?? []).map(
		(t) => t.textContent?.trim() ?? ''
	);
}

describe('home hero prefers shows with real art', () => {
	it('takes the first three shows with art, and the row leads with the ones it skipped', async () => {
		serve([
			bare('1', 'Hottest'),
			withBanner('2', 'Second'),
			bare('3', 'Third'),
			withCover('4', 'Fourth'),
			withCover('5', 'Fifth'),
			withBanner('6', 'Sixth'),
			bare('7', 'Seventh')
		]);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('Second'), { timeout: 3000 });
		expect(pagerTitles()).toEqual(dotLabels(['Second', 'Fourth', 'Fifth']));
		expect((target.querySelector('.hero .hero-img') as HTMLImageElement).classList).not.toContain(
			'fallback'
		);
		await vi.waitFor(() => expect(trendingRow()).toEqual(['Hottest', 'Third', 'Sixth', 'Seventh']));
	});

	it('fills the rotation with blurred posters when fewer than three shows have art', async () => {
		serve([
			bare('1', 'Hottest'),
			bare('2', 'Second'),
			withCover('3', 'Third'),
			bare('4', 'Fourth'),
			bare('5', 'Fifth')
		]);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('Third'), { timeout: 3000 });
		expect(pagerTitles()).toEqual(dotLabels(['Third', 'Hottest', 'Second']));
		(target.querySelectorAll('.hero-pager-dot')[1] as HTMLButtonElement).click();
		await vi.waitFor(() => expect(heroTitle()).toBe('Hottest'));
		await vi.waitFor(() =>
			expect(
				(target.querySelector('.hero .hero-img') as HTMLImageElement).classList.contains('fallback')
			).toBe(true)
		);
		await vi.waitFor(() => expect(trendingRow()).toEqual(['Fourth', 'Fifth']));
	});

	it('shows a show the trending list repeats only once, in its first place', async () => {
		serve([
			withBanner('1', 'Hottest'),
			withCover('1', 'Hottest'),
			withCover('2', 'Second'),
			bare('3', 'Third'),
			withCover('4', 'Fourth'),
			withCover('2', 'Second'),
			bare('5', 'Fifth'),
			bare('3', 'Third')
		]);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('Hottest'), { timeout: 3000 });
		expect(pagerTitles()).toEqual(dotLabels(['Hottest', 'Second', 'Fourth']));
		await vi.waitFor(() => expect(trendingRow()).toEqual(['Third', 'Fifth']));
	});
});
