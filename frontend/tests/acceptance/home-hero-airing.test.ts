// Acceptance: Home's hero is the backend's pick (backend/src/hero),
// made from the trending shows the availability filter kept. With the
// airing preference off — the default, and what a release ships — the
// hero is the first three trending shows with art, as before the
// preference existed. With it on, the airing shows with art lead it,
// and the shows they displaced lead the trending row. When the pick
// cannot be had, the trending row fails as a failed trending fetch
// does: the hero says the catalogue is unreachable.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef } from './home-handlers';
import { heroBackend } from './hero-backend';

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

import type { AvailabilityBatchResponse, KitsuAnimeRef } from '../../src/lib/api';
import { errorAnswer } from '../answers';

function show(id: string, title: string, status: string): KitsuAnimeRef {
	return {
		...kitsuRef(id, title, 12),
		status,
		cover_image: {
			tiny: null,
			small: null,
			large: `https://media.kitsu.app/anime/${id}/cover_image/large.jpg`,
			original: null
		}
	};
}

const TRENDING = [
	show('1', 'First Finished', 'finished'),
	show('2', 'Second Finished', 'finished'),
	show('3', 'Third Finished', 'finished'),
	show('4', 'Airing Now', 'current')
];

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

function serve(airingFirst: boolean, extra: Parameters<typeof homeHandlers>[1] = []) {
	const backend = heroBackend({ airingFirst });
	server.use(
		...homeHandlers({}, [
			...extra,
			...backend.handlers,
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () =>
				HttpResponse.json<KitsuAnimeRef[]>(TRENDING)
			)
		])
	);
	return backend;
}

describe("home hero's airing preference", () => {
	it('off, the hero is the first three shows with art, in trending order', async () => {
		serve(false);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('First Finished'), { timeout: 3000 });
		expect(pagerTitles()).toEqual(
			dotLabels(['First Finished', 'Second Finished', 'Third Finished'])
		);
		await vi.waitFor(() => expect(trendingRow()).toEqual(['Airing Now']));
	});

	it('on, the airing show leads the hero, and the show it displaced leads the row', async () => {
		serve(true);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('Airing Now'), { timeout: 3000 });
		expect(pagerTitles()).toEqual(dotLabels(['Airing Now', 'First Finished', 'Second Finished']));
		await vi.waitFor(() => expect(trendingRow()).toEqual(['Third Finished']));
	});

	it('asks for the pick over the shows the availability filter kept, in trending order', async () => {
		const backend = serve(true, [
			http.post(`${API_BASE}/api/availability/batch`, () =>
				HttpResponse.json<AvailabilityBatchResponse>({
					cached: { '2': false },
					playable_episode_counts: {}
				})
			),
			http.post(`${API_BASE}/api/availability/warm`, () => new HttpResponse(null, { status: 202 }))
		]);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe('Airing Now'), { timeout: 3000 });
		expect(backend.bodies.map((b) => b.shows.map((s) => s.id))).toEqual([['1', '3', '4']]);
		expect(backend.bodies[0].shows[2]).toEqual({
			id: '4',
			title: 'Airing Now',
			status: 'current',
			anilist_banner: null,
			cover_image: {
				large: 'https://media.kitsu.app/anime/4/cover_image/large.jpg',
				original: null,
				small: null
			}
		});
		expect(pagerTitles()).toEqual(dotLabels(['Airing Now', 'First Finished', 'Third Finished']));
	});

	it('says the catalogue is unreachable when the pick cannot be had', async () => {
		serve(false, [http.post(`${API_BASE}/api/home/hero`, () => errorAnswer(500))]);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe(m.home_hero_error_title()), {
			timeout: 3000
		});
		expect(trendingRow()).toEqual([]);
	});
});
