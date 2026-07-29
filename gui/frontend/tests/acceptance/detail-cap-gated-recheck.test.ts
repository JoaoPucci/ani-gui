// Acceptance: clicking a dimmed episode actually asks allmanga again.
//
// A tile dims when the anime database says the episode aired but it
// sits above the episode count allmanga reported. That count is
// cached — 24 hours for an ongoing show — and allmanga catalogues
// episodes inside that window, so the tile can stay dead long after
// the episode became streamable.
//
// This is the scenario the first version of the feature needed and
// did not have. The controller's own cases hand it a stand-in for the
// lookup, so they proved the state machine and nothing about the
// boundary it exists to cross: the request went out WITHOUT asking to
// skip the cached row, the lookup answered from the very row being
// questioned, and the tile reported "still not available" having
// reached nobody. Every unit case stayed green.
//
// So the assertion here is deliberately about the request, not the
// verdict: a second availability call has to leave the app, and it
// has to carry the instruction to ignore what is remembered.

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
const TITLE = 'Ongoing Show';
/** The anime database says 5 have aired. */
const AIRED = 5;
/** allmanga's cached answer says it only has 4 — so 5 is dimmed. */
const CACHED_COUNT = 4;

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

/** The blocking overlay the page raises for any long action. */
const overlay = () => target.querySelector('[role="status"].backdrop');

/** The tile for episode `n`, whatever state it is in. */
const tile = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;

describe('detail route — clicking a dimmed aired episode', () => {
	it('asks allmanga again instead of replaying the remembered count', async () => {
		const probes: { bypass_cache?: boolean }[] = [];

		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
			),
			// The database's view: five episodes are out.
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({
					aired: AIRED,
					next_episode: AIRED + 1,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				probes.push(body);
				return HttpResponse.json({
					available: true,
					// Every answer is the stale one. If the click reaches
					// allmanga the request itself proves it; the count
					// staying short keeps this scenario about the request
					// rather than about playback starting.
					episode_count: CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });

		// Episode 5 aired but is past allmanga's count, so it renders
		// dimmed — the state this whole feature is about.
		const bypassing = () => probes.filter((p) => p.bypass_cache === true);

		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		await until(() => probes.length > 0, 'the page-load availability lookup');

		// Counting requests would pass on nothing but a late mount-time
		// lookup landing after the snapshot. The bypass flag is what
		// only the re-ask sets, so it identifies the request rather
		// than merely noticing that one more arrived.
		expect(bypassing()).toHaveLength(0);

		tile(AIRED)!.click();

		// The two defects this pins, together: the click has to produce
		// a request at all, and that request has to say "not what you
		// remember" — without which the lookup answers from the very
		// row that dimmed the tile and reaches allmanga never.
		await until(() => bypassing().length > 0, 'the click to send a cache-skipping lookup');
	});

	it('blocks the page while it checks, like every other long action here', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => {
			release = r;
		});

		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({
					aired: AIRED,
					next_episode: AIRED + 1,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			// The page-load lookup answers at once; the one the click
			// sends is held open. Racing a poll against a lookup that
			// resolves in under a millisecond would only ever catch the
			// overlay by luck — held open, "the page is blocked" is a
			// state rather than a window.
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				if (body.bypass_cache) await held;
				return HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		expect(overlay()).toBeNull();

		tile(AIRED)!.click();

		// Every other long operation on this page raises the overlay and
		// holds the user until it resolves. The re-ask was built beside
		// that convention rather than inside it, and the whole family of
		// review findings — a second tile clicked, a navigation, an
		// episode-page change, the busy marker moving — lives in the
		// window that leaves open. Blocking closes the window.
		await until(() => overlay() !== null, 'the page to block while it checks');

		// And it lets go again once the answer lands short, rather than
		// stranding the user under an overlay that never clears.
		release();
		await until(() => overlay() === null, 'the page to unblock once the check came back short');
	});

	it('presents the dimmed tile as something to click, not as refused', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({
					aired: AIRED,
					next_episode: AIRED + 1,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				})
			)
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);

		// The tooltip is what distinguishes a dimmed-but-clickable tile
		// from an unaired or uncatalogued one, so it doubles as the
		// signal that the cap has actually landed and the tile is in
		// the state this scenario is about.
		await until(
			() => tile(AIRED)!.getAttribute('title') === m.detail_ep_recheck_idle(),
			'the tile to settle into its cap-gated resting state'
		);

		// Dimmed says "not right now"; not-allowed says "never, stop
		// trying". The tile is a live control that re-asks allmanga, so
		// it must not wear the styling reserved for tiles with nothing
		// behind them — that is precisely the affordance the whole
		// feature adds.
		expect(tile(AIRED)!.classList.contains('ep-tile-disabled')).toBe(false);
	});
});
