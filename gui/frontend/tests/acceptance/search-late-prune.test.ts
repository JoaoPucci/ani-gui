// Acceptance: the two remaining /search render-then-prune cases from
// #111 — every result pruned, and a superseded run.
//
// Both are sequences of DOM states rather than single answers, which
// is why they were deferred to this tier. The superseded case in
// particular cannot be reached below it: the runner distinguishes runs
// by a generation token precisely so that re-running the SAME query
// text (A → B → A) still silences the first A, and a test that only
// calls a helper twice cannot tell that apart from a text comparison.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setQuery } from './page-state.svelte';

vi.mock('$app/state', () => ({
	get page() {
		return page;
	}
}));

import SearchPage from '../../src/routes/search/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

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

const CONFIG = {
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
};

/** Handlers every scenario here needs: settings plus the two rails the
 *  route fetches on mount, which the setup would otherwise reject. */
function baseHandlers() {
	return [
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(CONFIG)),
		http.get(`${API_BASE}/api/kitsu/trending`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json({ cached: {}, playable_episode_counts: {} })
		)
	];
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

const screen = () => target.textContent ?? '';

describe('/search late prune', () => {
	// PINS CURRENT BEHAVIOUR, and it is not the behaviour I expected.
	//
	// `results` holds the list AFTER availability pruning, so when the
	// gate rejects everything the template takes its
	// `results.length === 0` branch — the "nothing matched your query"
	// copy. Kitsu did match; availability is what emptied the page, and
	// the message sends the user off to retype their query in romaji.
	//
	// `search_filtered_empty` is NOT this state: it needs
	// `results.length > 0`, so it only fires when the subtype chips
	// filter out what availability left. Filed as a follow-up rather
	// than fixed here — this PR adds coverage, and changing user-facing
	// copy is its own change with its own locales.
	it('falls back to the no-results copy when availability prunes everything', async () => {
		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search`, () =>
				HttpResponse.json([ref('1', 'Gone One'), ref('2', 'Gone Two')])
			),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({ available: false, episode_count: 0, approximate: false })
			)
		);

		setQuery('gone');
		app = mount(SearchPage, { target });

		await until(
			() => screen().includes('Nothing matched'),
			'an empty state once every probe rejected'
		);
		expect(screen()).not.toContain('Gone One');
		expect(screen()).not.toContain('Gone Two');
		// The distinction this pins: the page cannot currently say
		// "found, but unavailable", so the availability-filter copy
		// never appears on this path.
		expect(screen()).not.toContain('No matches in the current filter.');
	});

	it('lets the newest run win when the same query is re-submitted mid-flight', async () => {
		// A → B → A. The first A is still in flight when the second
		// lands, and both carry the identical query string, so nothing
		// about the text distinguishes them.
		let releaseFirstA: () => void = () => {};
		const firstAHeld = new Promise<void>((resolve) => {
			releaseFirstA = resolve;
		});
		let searchCalls = 0;

		server.use(
			...baseHandlers(),
			http.post(`${API_BASE}/api/kitsu/search`, async ({ request }) => {
				const { query } = (await request.json()) as { query: string };
				searchCalls += 1;
				if (query === 'cowboy' && searchCalls === 1) {
					// The stale run: held open, then answered with
					// results that must never reach the screen.
					await firstAHeld;
					return HttpResponse.json([ref('9', 'Stale Result')]);
				}
				if (query === 'trigun') return HttpResponse.json([ref('5', 'Interim Result')]);
				return HttpResponse.json([ref('7', 'Fresh Result')]);
			}),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({ available: true, episode_count: 12, approximate: false })
			)
		);

		setQuery('cowboy');
		app = mount(SearchPage, { target });
		await until(() => searchCalls === 1, 'the first run to reach the backend');

		setQuery('trigun');
		await until(() => screen().includes('Interim Result'), 'the interim run to render');

		setQuery('cowboy');
		await until(() => screen().includes('Fresh Result'), 're-running the original query to render');

		// Only now does the very first run answer. Its generation is
		// two behind, so its results are dropped rather than painted
		// over the ones the user is looking at.
		releaseFirstA();
		await new Promise((r) => setTimeout(r, 200));

		expect(screen()).not.toContain('Stale Result');
		expect(screen()).toContain('Fresh Result');
	});
});
