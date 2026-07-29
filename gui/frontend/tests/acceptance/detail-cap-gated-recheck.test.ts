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

	it('does not play a sub answer once settings land on dub', async () => {
		let releaseSettings!: () => void;
		const settingsHeld = new Promise<void>((r) => {
			releaseSettings = r;
		});
		let releaseRecheck!: () => void;
		const recheckHeld = new Promise<void>((r) => {
			releaseRecheck = r;
		});
		const played: { episode?: string; prefetch?: boolean }[] = [];
		const probes: { bypass_cache?: boolean; mode?: string }[] = [];

		server.use(
			// Settings arrive after the page does — the gap a click can
			// land in. Held open here rather than raced, so the ordering
			// is the test's rather than the scheduler's.
			http.get(`${API_BASE}/api/settings`, async () => {
				await settingsHeld;
				return HttpResponse.json({ ...appConfig(), mode: 'dub' });
			}),
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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean; mode?: string };
				probes.push(body);
				if (body.bypass_cache) await recheckHeld;
				return HttpResponse.json({
					available: true,
					// Reaches episode 5 — but it is the SUB count, because
					// that is what the click asked with.
					episode_count: body.bypass_cache ? AIRED + 2 : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			}),
			http.post(`${API_BASE}/api/play`, async ({ request }) => {
				played.push((await request.json()) as { episode?: string; prefetch?: boolean });
				return HttpResponse.json({
					id: 'session-1',
					kind: 'hls',
					has_subtitles: false,
					quality: '1080',
					mode: 'sub'
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		await until(() => probes.length > 0, 'the page-load availability lookup');

		tile(AIRED)!.click();
		await until(
			() => probes.some((p) => p.bypass_cache === true),
			'the click to send a cache-skipping lookup'
		);
		expect(probes.find((p) => p.bypass_cache)?.mode).toBe('sub');

		// The real mode turns up while the answer is still out, then the
		// answer lands.
		releaseSettings();
		await until(() => tile(AIRED) !== null, 'the page to survive the settings arriving');
		releaseRecheck();

		// Either outcome ends the wait: dropping the answer releases the
		// page, and applying it starts a play. Waiting on the release
		// alone would let the wrong behaviour report itself as a
		// timeout rather than as the thing it actually did.
		await until(
			() => overlay() === null || played.length > 0,
			'the page to either unblock or start playing'
		);

		// A sub count of 7 says nothing about what the dub catalogue
		// has. Playing on it resolves an episode that may not exist.
		expect(played.filter((p) => p.episode === String(AIRED) && !p.prefetch)).toEqual([]);
	});

	it('splices in a special the recheck found, even when the episode stays gated', async () => {
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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				return HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					// The page loaded knowing of no specials. Asking
					// allmanga directly turns up one catalogued since.
					extra_episodes: body.bypass_cache ? ['4.5'] : [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		expect(target.querySelector('li[data-ep-num="4.5"]')).toBeNull();

		tile(AIRED)!.click();

		// The clicked episode is still out of reach — the count came
		// back the same. That must not stop the rest of the fresh row
		// from landing: the re-ask replaced the whole cached row, and
		// the strip splices non-integer tags in at their numeric
		// position, so this special is invisible until it is applied.
		await until(
			() => target.querySelector('li[data-ep-num="4.5"]') !== null,
			'the newly catalogued special to appear in the strip'
		);
	});

	it('stops re-asking once the answer was that the show is gone', async () => {
		const probes: { bypass_cache?: boolean }[] = [];
		let delisted = false;

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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				probes.push(body);
				if (body.bypass_cache) delisted = true;
				return HttpResponse.json({
					available: !delisted,
					episode_count: delisted ? null : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		await until(() => probes.length > 0, 'the page-load availability lookup');

		const bypassing = () => probes.filter((p) => p.bypass_cache === true);
		tile(AIRED)!.click();
		await until(() => bypassing().length === 1, 'the first re-ask');
		await until(
			() => tile(AIRED)?.classList.contains('ep-tile-disabled') === true,
			'the tile to go unavailable once the show is delisted'
		);

		// The confirmed cap of zero makes EVERY tile cap-gated, so the
		// recheck styling and tooltip would otherwise win over the
		// unavailable ones — a pointer cursor and "click to check
		// again" on a tile whose click is now, correctly, inert.
		expect(tile(AIRED)!.classList.contains('ep-tile-recheck')).toBe(false);
		expect(tile(AIRED)!.getAttribute('title')).toBe(m.detail_ep_disabled_tooltip());

		// `aria-disabled` is advisory — it does not stop a click. The
		// tile is still cap-gated too, because a delisting arrives
		// without a count, so a handler that checks cap-gated first
		// sends allmanga the same question again about a show it has
		// just said it does not have.
		tile(AIRED)!.click();

		// A bounded absence, which is sound here because the defect is
		// synchronous: the click handler raises the overlay and calls
		// the lookup in the same turn, so if it were going to happen it
		// would have by now.
		await new Promise((r) => setTimeout(r, 300));
		expect(bypassing()).toHaveLength(1);
		expect(overlay()).toBeNull();
	});

	it('does not drag the user back to playback after they leave the page', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => {
			release = r;
		});
		const played: { episode?: string; prefetch?: boolean }[] = [];

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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				if (body.bypass_cache) await held;
				return HttpResponse.json({
					available: true,
					// The remembered count gates the tile; the held re-ask
					// comes back reaching it, so the answer would clear
					// the gate and start playback if anything still acted
					// on it.
					episode_count: body.bypass_cache ? AIRED + 1 : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			}),
			http.post(`${API_BASE}/api/play`, async ({ request }) => {
				played.push((await request.json()) as { episode?: string; prefetch?: boolean });
				return HttpResponse.json({
					id: 'session-1',
					kind: 'hls',
					has_subtitles: false,
					quality: '1080',
					mode: 'sub'
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		tile(AIRED)!.click();
		await until(() => overlay() !== null, 'the page to block while it checks');

		// The user leaves — browser Back, or any history navigation.
		// The show and the mode are unchanged, so the context guard
		// sees nothing wrong; only the component is gone.
		unmount(app);
		app = null;

		release();
		await new Promise((r) => setTimeout(r, 300));

		// Clearing the gate here calls startPlay, whose goto would haul
		// the user back into playback on a page they already left.
		expect(played.filter((p) => p.episode === String(AIRED) && !p.prefetch)).toEqual([]);
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

	it('does not revive an abandoned re-ask when the user comes back to the show', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => {
			release = r;
		});
		const played: { episode?: string; prefetch?: boolean }[] = [];
		const OTHER_ID = '43';
		const OTHER_TITLE = 'Another Show';

		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
				HttpResponse.json({
					...kitsuRef(String(params.id), String(params.id) === OTHER_ID ? OTHER_TITLE : TITLE, 12),
					status: 'current'
				})
			),
			http.get(`${API_BASE}/api/kitsu/airing/:id`, () =>
				HttpResponse.json({
					aired: AIRED,
					next_episode: AIRED + 1,
					next_airing_at: null,
					upcoming: []
				})
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			// Two shows are visited, so the per-show reads have to answer
			// for either id rather than for the one the other scenarios
			// name explicitly.
			http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json([])),
			http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json({ dir: '/tmp' })),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/play/mark-watched`, () => HttpResponse.json({ ok: true })),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				if (body.bypass_cache) await held;
				return HttpResponse.json({
					available: true,
					episode_count: body.bypass_cache ? AIRED + 1 : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			}),
			http.post(`${API_BASE}/api/play`, async ({ request }) => {
				played.push((await request.json()) as { episode?: string; prefetch?: boolean });
				return HttpResponse.json({
					id: 'session-1',
					kind: 'hls',
					has_subtitles: false,
					quality: '1080',
					mode: 'sub'
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		tile(AIRED)!.click();
		await until(() => overlay() !== null, 'the page to block while it checks');

		// The user leaves for another show and comes back. SvelteKit
		// reuses the component for both, so nothing was destroyed and
		// the show and mode are the same two strings they were when the
		// click went out — the answer is about a page that no longer
		// holds the click that asked for it.
		setParams({ id: OTHER_ID });
		await until(
			() => (target.textContent ?? '').includes(OTHER_TITLE),
			'the page to move on to the other show'
		);
		setParams({ id: KITSU_ID });
		await until(
			() => (target.textContent ?? '').includes(TITLE) && tile(AIRED) !== null,
			`the tile for episode ${AIRED} on the show the user came back to`
		);

		release();
		await new Promise((r) => setTimeout(r, 300));

		expect(played.filter((p) => p.episode === String(AIRED) && !p.prefetch)).toEqual([]);
	});

	it('asks again for the mode that settings landed on', async () => {
		let releaseSettings!: () => void;
		const settingsHeld = new Promise<void>((r) => {
			releaseSettings = r;
		});
		let releaseSub!: () => void;
		const subHeld = new Promise<void>((r) => {
			releaseSub = r;
		});
		const probes: { bypass_cache?: boolean; mode?: string }[] = [];

		server.use(
			http.get(`${API_BASE}/api/settings`, async () => {
				await settingsHeld;
				return HttpResponse.json({ ...appConfig(), mode: 'dub' });
			}),
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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean; mode?: string };
				probes.push(body);
				// The fallback lookup is still out when the real mode
				// arrives, which is the whole ordering being tested.
				if (body.mode === 'sub') await subHeld;
				return HttpResponse.json({
					available: true,
					// Dub lags, as it does upstream: the sub catalogue has
					// everything that aired, the dub catalogue does not.
					episode_count: body.mode === 'dub' ? CACHED_COUNT : AIRED,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => tile(AIRED) !== null, `the tile for episode ${AIRED}`);
		await until(() => probes.length > 0, 'the fallback availability lookup');
		expect(probes[0].mode).toBe('sub');

		// Settings land on dub while that first lookup is still out. Its
		// answer is about the wrong catalogue and is correctly dropped —
		// so something has to ask again, or the page keeps no cap at all
		// and every aired dub tile stays playable.
		releaseSettings();
		releaseSub();

		await until(
			() => probes.some((p) => p.mode === 'dub' && !p.bypass_cache),
			'a second lookup for the mode the user actually has',
			3000
		);
		await until(
			() => tile(AIRED)!.getAttribute('title') === m.detail_ep_recheck_idle(),
			`episode ${AIRED} to be gated by the dub cap`,
			3000
		);

		expect(probes.filter((p) => p.mode === 'dub')).toHaveLength(1);
	});

	it('does not ask twice when settings confirm the mode it guessed', async () => {
		let releaseSettings!: () => void;
		const settingsHeld = new Promise<void>((r) => {
			releaseSettings = r;
		});
		const probes: { bypass_cache?: boolean; mode?: string }[] = [];

		server.use(
			http.get(`${API_BASE}/api/settings`, async () => {
				await settingsHeld;
				return HttpResponse.json({ ...appConfig(), mode: 'sub' });
			}),
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
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean; mode?: string };
				probes.push(body);
				return HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => probes.length > 0, 'the fallback availability lookup');

		// Settings confirm what the fallback already guessed. The mode
		// did not change, so nothing about the question did — and
		// allmanga rate-limits, so asking it twice costs a slot for an
		// answer already in hand.
		releaseSettings();
		await until(
			() => tile(AIRED)!.getAttribute('title') === m.detail_ep_recheck_idle(),
			`episode ${AIRED} to settle under the cap`,
			3000
		);
		await new Promise((r) => setTimeout(r, 200));

		expect(probes.filter((p) => !p.bypass_cache)).toHaveLength(1);
	});
});
