// Acceptance: Home asks the backend once, as its history loads, to warm
// the whole Continue strip's stills and banners — the Kitsu ids its
// rows recorded, in strip order — instead of leaving each card to ask
// AniList on its own as its row becomes ready.

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

import type { KitsuAnimeRef, KitsuEpisode } from '../../src/lib/api';

const ROWS = [
	{ ep_no: '3', id: 'hianime:show-a-1', title: 'Show A', kitsu_id: '101' },
	{ ep_no: '1', id: 'hianime:show-b-2', title: 'Show B' },
	{ ep_no: '5', id: 'hianime:show-c-3', title: 'Show C', kitsu_id: '103' }
];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
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

describe("Home's Continue strip", () => {
	it('asks the backend once to warm the recorded shows, before any card asks for its episodes', async () => {
		const order: string[] = [];
		const warms: unknown[] = [];
		server.use(
			...homeHandlers({ history: ROWS }, [
				http.post(`${API_BASE}/api/continue/warm`, async ({ request }) => {
					order.push('warm');
					warms.push(await request.json());
					return new HttpResponse(null, { status: 204 });
				}),
				http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
					HttpResponse.json<KitsuAnimeRef>(kitsuRef(String(params.id), `Show ${params.id}`, 12))
				),
				http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => {
					order.push('episodes');
					return HttpResponse.json<KitsuEpisode[]>([]);
				}),
				http.post(`${API_BASE}/api/kitsu/search`, () =>
					HttpResponse.json<KitsuAnimeRef[]>([kitsuRef('102', 'Show B', 12)])
				)
			])
		);
		app = mount(HomePage, { target });
		await until(() => warms.length > 0, 'the strip warm');
		await until(() => order.includes('episodes'), "a card's episodes");
		expect(warms).toEqual([{ kitsu_ids: ['101', '103'] }]);
		expect(order[0]).toBe('warm');
	});
});
