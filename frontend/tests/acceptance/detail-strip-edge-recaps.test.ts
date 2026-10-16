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
	});
});
