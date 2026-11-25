// Acceptance: while a show's episodes load, the episode grid tells a
// screen reader it is loading.
//
// The grid holds placeholder tiles until Kitsu's episode list lands.
// The tiles are hidden from assistive tech (they are shapes, not
// episodes), so the grid itself carries the state: busy while the list
// is out, and no longer once the episodes are in it.

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

import type { AvailabilityResponse, Config, KitsuAnimeRef, KitsuEpisode } from '../../src/lib/api';
import { availabilityAnswer } from '../answers';

const KITSU_ID = '41370';
const TITLE = 'Held Episodes Show';

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

function episode(n: number): KitsuEpisode {
	return {
		id: `ep-${n}`,
		number: n,
		relative_number: n,
		season_number: 1,
		canonical_title: `Episode ${n}`,
		synopsis: null,
		airdate: '2020-01-01',
		length: 24,
		thumbnail: null
	} as KitsuEpisode;
}

const grid = () => target.querySelector('ul.ep-grid');

describe('detail route — the episode grid while its episodes load', () => {
	it('is busy while the list is out, and not once it lands', async () => {
		let release: () => void = () => {};
		const gate = new Promise<void>((r) => (release = r));
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef(KITSU_ID, TITLE, 3))
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, async () => {
				await gate;
				return HttpResponse.json<KitsuEpisode[]>([episode(1), episode(2), episode(3)]);
			}),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json<AvailabilityResponse>(
					availabilityAnswer({ available: true, episode_count: 3, extra_episodes: [] })
				)
			)
		);

		app = mount(DetailPage, { target });
		await until(() => target.querySelector('.ep-tile-skel') !== null, 'the placeholder tiles');
		expect(grid()?.getAttribute('aria-busy')).toBe('true');

		release();
		await until(() => target.querySelector('li[data-ep-num="3"]') !== null, 'the episodes');
		expect(target.querySelector('.ep-tile-skel')).toBeNull();
		expect(grid()?.getAttribute('aria-busy')).not.toBe('true');
	});
});
