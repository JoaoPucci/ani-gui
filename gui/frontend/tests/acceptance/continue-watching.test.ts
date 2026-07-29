// Acceptance: the Continue Watching early-click flow on the home
// route — the scenarios #112 deferred to this tier.
//
// The home page bootstraps `settingsGet()` and `historyList()` in
// parallel, so the availability mode is not known when the first
// history rows arrive. `continue-watching-loader.ts` holds per-row
// probes until `getMode()` resolves for that reason: a dub user
// probed under the 'sub' fallback reads the wrong playable count,
// while the click path later uses the real mode. The race is between
// two in-flight requests, so it only exists once the route, the
// loader and the API layer are running together.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';

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

function ref(id: string, title: string, episodeCount: number) {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: episodeCount,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null
	};
}

function config(mode: 'sub' | 'dub') {
	return {
		locale: 'en',
		mode,
		quality: 'best',
		external_player: '',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: '',
		image_cache_cap_mb: 100,
		auto_play_next: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: true,
		disable_auto_pip_on_leave: false,
		auto_update_anicli: false,
		update_include_prereleases: false,
		primary_account: ''
	};
}

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

/** The Continue row's resolved card. The loading branch is a
 *  non-interactive `div`, so a button carrying the show's name exists
 *  only once the row's Kitsu match has landed. */
const resumeButton = () =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		b.textContent?.includes('Cowboy Bebop')
	) ?? null;

describe('home Continue Watching', () => {
	it('probes availability under the configured mode, not the sub fallback', async () => {
		const probedModes: string[] = [];
		let releaseSettings: () => void = () => {};
		const settingsHeld = new Promise<void>((resolve) => {
			releaseSettings = resolve;
		});

		server.use(
			// Held so history lands first — the ordering the loader has
			// to survive. Answering immediately would let the mode be
			// known before any row arrived and prove nothing.
			http.get(`${API_BASE}/api/settings`, async () => {
				await settingsHeld;
				return HttpResponse.json(config('dub'));
			}),
			http.get(`${API_BASE}/api/history`, () =>
				HttpResponse.json([{ ep_no: '3', id: 'allanime-1', title: 'Cowboy Bebop' }])
			),
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([ref('1', 'Cowboy Bebop', 26)])
			),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { mode?: string };
				probedModes.push(body.mode ?? '(none)');
				return HttpResponse.json({ available: true, episode_count: 26, approximate: false });
			}),
			http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),

			// The rest of what a mounted home route reaches for. Every
			// one is stubbed so `onUnhandledRequest: 'error'` keeps its
			// meaning: without them the route still reaches availability,
			// but by way of its failure fallbacks rather than the
			// cold-cache resolution this scenario describes.
			http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
			// Cold caches: no stored allanime→Kitsu mapping and no
			// remembered title match, so resolution goes through the
			// Kitsu search above and writes its result back.
			http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(null)),
			http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([]))
		);

		app = mount(HomePage, { target });

		// The RESOLVED card, not the title. While `match` is undefined
		// the row renders a loading placeholder that already shows the
		// history entry's own title, so waiting on text would fire
		// before any Kitsu matching happened. Only the resolved branch
		// renders an interactive button — matching is finished by the
		// time one exists.
		await until(() => resumeButton() !== null, 'the row to resolve into an interactive card');
		expect(probedModes).toEqual([]);

		releaseSettings();

		await until(() => probedModes.length > 0, 'the row to be probed once settings resolved');

		// The assertion that carries the weight, and the one that needs
		// no ordering luck: whatever probes happen, they all carry the
		// configured mode. An ungated loader reads the 'sub' fallback,
		// so its probes fail this whenever they are issued.
		expect(probedModes.every((m) => m === 'dub')).toBe(true);
	});
});
