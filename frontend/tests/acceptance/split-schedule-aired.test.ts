// Acceptance: episodes the anime database dates as out are not greyed
// out because the schedule describes a different entry.
//
// Steel Ball Run, as the app saw it: the anime database keeps one
// twelve-episode entry whose first episode came out in March and whose
// second and third followed weekly from late September. The schedule
// source files the March release as an entry of its own — one episode,
// finished — and that is the entry the show maps to, so its aired count
// stops at one. The provider's remembered count is two.
//
// Followed alone, the schedule greyed episodes two and three as
// unaired, and an unaired tile does nothing when clicked — so the
// re-ask that a tile past the provider's count gets never ran either.
// Both routes have to read the dated episodes as the floor they are:
// episode two plays, episode three re-asks the provider, episode four
// is still to come.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '49847';
const TITLE = 'Steel Ball Run';

function episode(n: number, airdate: string | null) {
	return {
		id: `ep-${n}`,
		canonical_title: `Episode ${n}`,
		season_number: 1,
		number: n,
		relative_number: null,
		length: 47,
		synopsis: null,
		airdate,
		thumbnail: null
	};
}

/** What the anime database lists: three dated episodes. */
const DATED = [episode(1, '2026-03-19'), episode(2, '2026-09-25'), episode(3, '2026-10-02')];

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let probes: { bypass_cache?: boolean }[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	target = document.createElement('div');
	document.body.appendChild(target);
	probes = [];
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
		),
		// The schedule's entry: one episode, finished, nothing next.
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 1, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(DATED)),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			probes.push((await request.json()) as { bypass_cache?: boolean });
			return HttpResponse.json({
				available: true,
				episode_count: 2,
				extra_episodes: [],
				episode_count_approximate: false
			});
		}),
		http.post(`${API_BASE}/api/play`, () =>
			HttpResponse.json({
				id: 'session-1',
				kind: 'hls',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
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

const tile = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;
const bypassing = () => probes.filter((p) => p.bypass_cache === true);

describe('a schedule that stops short of the dated episodes', () => {
	it('detail route: two plays, three re-asks the provider, four is to come', async () => {
		app = mount(DetailPage, { target });
		await until(() => tile(4) !== null && probes.length > 0, 'the strip and the lookup');
		await until(() => tile(4)!.classList.contains('ep-tile-unaired'), 'the schedule to land');

		expect(tile(2)!.classList.contains('ep-tile-unaired')).toBe(false);
		expect(tile(3)!.classList.contains('ep-tile-unaired')).toBe(false);
		expect(tile(3)!.classList.contains('ep-tile-recheck')).toBe(true);

		tile(3)!.click();
		await until(() => bypassing().length > 0, 'the click to send a cache-skipping lookup');
	});

	it('play route: the same strip, the same re-ask', async () => {
		setUrl(`/play/${KITSU_ID}`, { episode: '1' });
		app = mount(PlayPage, { target });
		await until(() => tile(4) !== null && probes.length > 0, 'the strip and the lookup');
		await until(() => tile(4)!.classList.contains('ep-card-unaired'), 'the schedule to land');

		expect(tile(2)!.classList.contains('ep-card-unaired')).toBe(false);
		expect(tile(3)!.classList.contains('ep-card-unaired')).toBe(false);
		expect(tile(3)!.classList.contains('ep-card-recheck')).toBe(true);

		tile(3)!.click();
		await until(() => bypassing().length > 0, 'the click to send a cache-skipping lookup');
	});
});
