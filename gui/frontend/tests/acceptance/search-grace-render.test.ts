// Acceptance: /search renders every result the moment it has them,
// then prunes the ones the availability probes rejected.
//
// This is the transition #111 shipped and deferred a test for, and it
// is the reason this tier exists: the behaviour is a sequence of DOM
// states over time, composed from the search runner, the progressive
// availability filter, the probe pool and the route's own template.
// A unit test on any one of those cannot say whether the user sees
// results early, and a Playwright run cannot control probe timing
// finely enough to prove the middle state was ever on screen.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';

// `$app/state.page` is populated by SvelteKit's client router, which
// is not running here. The route reads exactly one thing from it —
// the `?q=` param it drives search off — so that is all this supplies.
const pageState = { url: new URL(`${API_BASE}/search?q=cowboy`) };
vi.mock('$app/state', () => ({
	get page() {
		return pageState;
	}
}));

import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

/** A Kitsu ref with only the fields the search surface reads. */
function ref(id: string, title: string) {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: 12,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null
	};
}

const AVAILABLE = ref('1', 'Available Show');
const MISSING = ref('2', 'Missing Show');

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

/** Poll the mounted DOM until `predicate` holds, or fail loudly. */
async function until(predicate: () => boolean, what: string, timeoutMs = 2000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

const titlesOnScreen = () => target.textContent ?? '';

describe('/search render-then-prune', () => {
	it('shows every result before the probes settle, then drops the unavailable one', async () => {
		let releaseProbes: () => void = () => {};
		const probesHeld = new Promise<void>((resolve) => {
			releaseProbes = resolve;
		});

		server.use(
			http.get(`${API_BASE}/api/settings`, () =>
				HttpResponse.json({
					locale: 'en',
					mode: 'sub',
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
				})
			),
			// The rails are not what this scenario is about, but the
			// route fetches them on mount and the setup errors on any
			// request it did not describe.
			http.get(`${API_BASE}/api/kitsu/trending`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),

			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([AVAILABLE, MISSING])),
			// Nothing cached, so both rows go to the inline probe pool.
			http.post(`${API_BASE}/api/availability/batch`, () =>
				HttpResponse.json({ cached: {}, playable_episode_counts: {} })
			),
			// Held open until the assertion on the grace render has
			// run. This is the hostage case the filter was written for:
			// probes that never come back must not hold the page.
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { kitsu_id?: string };
				await probesHeld;
				return HttpResponse.json({
					available: body.kitsu_id === AVAILABLE.id,
					episode_count: 12,
					approximate: false
				});
			})
		);

		app = mount(SearchPage, { target });

		// The render happens AT the filter's grace deadline (2s by
		// default), not immediately — that is the point of the design,
		// so the wait has to outlast it rather than race it.
		await until(
			() =>
				titlesOnScreen().includes('Available Show') && titlesOnScreen().includes('Missing Show'),
			'both results to render at the grace deadline, with every probe still in flight',
			6000
		);

		releaseProbes();

		await until(
			() => !titlesOnScreen().includes('Missing Show'),
			'the unavailable result to be pruned once its probe answered',
			6000
		);
		expect(titlesOnScreen()).toContain('Available Show');
	});
});
