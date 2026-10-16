// Acceptance: the detail page's episode grid shows a recap at the
// edge of a page on that page.
//
// Pages of twelve: a recap numbered 12.5 follows episode 12, and one
// numbered 0.5 comes before episode 1. Placed by their own numbers
// they fell between pages, or before every page, and showed nowhere.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';

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

const KITSU_ID = '42';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

function useShow(count: number, extras: string[], announced = count, airingKnown = true) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Ongoing Show', announced), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			airingKnown
				? HttpResponse.json({
						aired: count,
						next_episode: null,
						next_airing_at: null,
						upcoming: []
					})
				: new HttpResponse(null, { status: 404 })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: count,
				extra_episodes: extras,
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
}

describe('detail route — a recap does not make a short run look paged', () => {
	it('grows no pager for 11 episodes and a recap before episode 1', async () => {
		useShow(11, ['0.5']);
		app = mount(DetailPage, { target });
		await until(() => target.querySelector('li[data-ep-num="0.5"]') !== null, 'the grid');
		await new Promise((r) => setTimeout(r, 50));
		expect(target.querySelector('form.ep-jump')).toBeNull();
	});
});

describe('detail route — recaps at the edges of a grid page', () => {
	it('shows 0.5 and 12.5 on page 1', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Ongoing Show', 24), status: 'finished' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({ aired: 24, next_episode: null, next_airing_at: null, upcoming: [] })
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: 24,
					extra_episodes: ['0.5', '12.5'],
					episode_count_approximate: false
				})
			),
			http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
			http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
		);
		app = mount(DetailPage, { target });
		await until(() => target.querySelector('li[data-ep-num="1"]') !== null, 'the grid');
		await until(
			() => target.querySelector('li[data-ep-num="12.5"]') !== null,
			'the 12.5 tile on page 1'
		);
		expect(target.querySelector('li[data-ep-num="0.5"]')).not.toBeNull();
		// The range counts episodes, not the recaps beside them.
		expect(target.textContent).toContain(
			m.detail_episodes_range_pagination({ epStart: '1', epEnd: '12', total: '24' })
		);
	});
});

describe('detail route — a short listing with a recap', () => {
	it('counts the listed episodes, not the recap beside them', async () => {
		// The provider lists five episodes and a 5.5 recap of a show
		// Kitsu announces at 24, with no airing data to say more: the
		// range reads "5 of 24 listed", not 6.
		useShow(5, ['5.5'], 24, false);
		app = mount(DetailPage, { target });
		await until(() => target.querySelector('li[data-ep-num="5.5"]') !== null, 'the grid');
		await until(
			() =>
				(target.textContent ?? '').includes(
					m.detail_episodes_range_of({ aired: '5', total: '24' })
				),
			'the listed count'
		);
		expect(target.textContent).not.toContain(
			m.detail_episodes_range_of({ aired: '6', total: '24' })
		);
	});
});
