// Acceptance: Home's hero always ends somewhere honest. When the
// trending list comes back with no shows, or every show it names is
// filtered out as unavailable, the hero says there is nothing to
// feature and points at Search; when the fetch fails, it says the
// catalogue is unreachable. Neither stays on the loading skeleton.

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
import { m } from '../../src/lib/paraglide/messages';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	localStorage.clear();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	localStorage.clear();
});

const hero = () => target.querySelector('section.hero') as HTMLElement;
const heroTitle = () => hero()?.querySelector('.hero-title')?.textContent?.trim() ?? null;
const heroText = () => hero()?.textContent ?? '';
const busy = () => hero()?.querySelector('[aria-busy="true"]') !== null;
const searchLink = () =>
	Array.from(hero()?.querySelectorAll('a') ?? []).find((a) =>
		a.textContent?.includes(m.home_hero_error_cta())
	);

describe('home hero, when there is no show to feature', () => {
	it('says the trending list came back empty, and points at Search', async () => {
		server.use(...homeHandlers());
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe(m.home_hero_empty_title()), {
			timeout: 3000
		});
		expect(heroText()).toContain(m.home_hero_empty_body());
		expect(heroText()).toContain(m.home_hero_error_eyebrow_key());
		// The failure's "no answer" line would be false here: Kitsu answered.
		expect(heroText()).not.toContain(m.home_hero_error_eyebrow_value());
		expect(busy()).toBe(false);
		expect(searchLink()?.getAttribute('href')).toMatch(/\/search$/);
	});

	it('says the same when every trending show is filtered out as unavailable', async () => {
		const shows = [kitsuRef('1', 'Gone', 12), kitsuRef('2', 'Also gone', 12)];
		server.use(
			...homeHandlers({}, [
				http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json(shows)),
				http.post(`${API_BASE}/api/availability/batch`, () =>
					HttpResponse.json({ cached: { '1': false, '2': false }, playable_episode_counts: {} })
				),
				http.post(`${API_BASE}/api/availability/warm`, () => HttpResponse.json(null))
			])
		);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe(m.home_hero_empty_title()), {
			timeout: 3000
		});
		expect(busy()).toBe(false);
	});

	it('says the catalogue is unreachable when the trending fetch fails', async () => {
		server.use(
			...homeHandlers({}, [
				http.get(`${API_BASE}/api/kitsu/trending-anilist`, () =>
					HttpResponse.json(null, { status: 500 })
				)
			])
		);
		app = mount(HomePage, { target });
		await vi.waitFor(() => expect(heroTitle()).toBe(m.home_hero_error_title()), {
			timeout: 3000
		});
		expect(heroText()).toContain(m.home_hero_error_body());
		expect(busy()).toBe(false);
		expect(searchLink()?.getAttribute('href')).toMatch(/\/search$/);
	});
});
