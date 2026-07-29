// Acceptance: the same dimmed-episode click, on the player page.
//
// The two routes share the controller and nothing else. The detail
// page blocks on `actionBusy` and hands a cleared count to
// `startPlay`; the player blocks on `switchBusy` and hands it to
// `switchToEpisode`, whose strip is paginated five cards at a time
// and whose own gate re-checks the cap before it will move. A
// scenario that mounts only the detail route proves none of that.
//
// So this mounts `/play/[id]` and drives the whole path: a tile the
// catalogue's remembered count says is not there, a click, the
// re-ask that skips that count, and the switch that follows when the
// fresh count reaches the episode.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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

import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
/** The anime database says 5 have aired. */
const AIRED = 5;
/** allmanga's remembered answer says it only has 4 — so 5 is dimmed. */
const CACHED_COUNT = 4;
/** What allmanga says when actually asked. */
const FRESH_COUNT = 5;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	setUrl(`/play/${KITSU_ID}`, { episode: '1' });
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

/** The card for episode `n` in the player's five-wide strip. */
const card = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		relative_number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		titles: {},
		synopsis: null,
		thumbnail: null,
		length_minutes: 24,
		air_date: '2019-01-01'
	}));
}

describe('play route — clicking a dimmed aired episode', () => {
	it('re-asks allmanga and switches to the episode when the count clears', async () => {
		const probes: { bypass_cache?: boolean }[] = [];
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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				probes.push(body);
				// The remembered count is short; asking allmanga directly
				// gets the episode that landed since.
				return HttpResponse.json({
					available: true,
					episode_count: body.bypass_cache ? FRESH_COUNT : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			}),
			http.post(`${API_BASE}/api/play`, async ({ request }) => {
				const body = (await request.json()) as { episode?: string; prefetch?: boolean };
				played.push(body);
				return HttpResponse.json({
					id: 'session-1',
					kind: 'hls',
					has_subtitles: false,
					quality: '1080',
					mode: 'sub'
				});
			}),
			// A completed switch marks the episode watched; the strip
			// also warms neighbours in the background. Neither is what
			// this scenario asserts, but both have to be answered or the
			// tier's unhandled-request error fires.
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		app = mount(PlayPage, { target });

		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);
		await until(() => probes.length > 0, 'the page-load availability lookup');

		const bypassing = () => probes.filter((p) => p.bypass_cache === true);
		expect(bypassing()).toHaveLength(0);

		card(AIRED)!.click();

		// The click has to reach allmanga past the remembered count —
		// asking with the cache still in play answers from the very
		// number that dimmed the card.
		await until(() => bypassing().length > 0, 'the click to send a cache-skipping lookup');

		// And the fresh count has to actually move the player. The
		// detail route reaches this through startPlay; here it is
		// switchToEpisode, which re-checks the cap itself and would
		// refuse the switch if the cleared count had not been published
		// first.
		// `prefetch` excluded: the strip warms neighbours on its own, so
		// a prefetch for this episode would satisfy a bare
		// episode-number check without the user's click having moved
		// anything.
		await until(
			() => played.some((p) => p.episode === String(AIRED) && !p.prefetch),
			`the player to switch to episode ${AIRED}`
		);
	});

	it('splices a refreshed special into the strip, and drops one that went away', async () => {
		let recheckExtras: string[] = ['4.5'];

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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				return HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					// The strip loaded knowing of no specials.
					extra_episodes: body.bypass_cache ? recheckExtras : [],
					episode_count_approximate: false
				});
			}),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
		);

		app = mount(PlayPage, { target });
		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);
		expect(target.querySelector('li[data-ep-num="4.5"]')).toBeNull();

		card(AIRED)!.click();

		// The player builds its own five-wide strip, so the detail
		// suite proves nothing about this splice.
		await until(
			() => target.querySelector('li[data-ep-num="4.5"]') !== null,
			'the newly catalogued special to appear in the player strip'
		);

		// And the other direction, which is the one that leaves a dead
		// card on screen: allmanga pulls the special, and the strip has
		// to stop offering it.
		recheckExtras = [];
		card(AIRED)!.click();
		await until(
			() => target.querySelector('li[data-ep-num="4.5"]') === null,
			'the withdrawn special to leave the player strip'
		);
	});

	it('stops offering any episode once the recheck says the show is gone', async () => {
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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				// The re-ask finds the show delisted — allmanga dropped
				// it, or the resolver corrected which title it matched.
				// No count comes with that, so the stale cap survives.
				if (body.bypass_cache) delisted = true;
				return HttpResponse.json({
					available: !delisted,
					episode_count: delisted ? null : CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				});
			})
		);

		app = mount(PlayPage, { target });
		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);
		// An episode comfortably inside the remembered cap, so nothing
		// about IT changes — only the show's existence does.
		await until(() => card(3)?.disabled === false, 'episode 3 to be offered at all');

		card(AIRED)!.click();

		// The verdict is the only thing this answer established: no
		// count came back, so the cap stays where it was and every
		// episode at or below it still looks playable. Clicking one
		// starts a resolution against a show that is not there.
		await until(
			() => card(3)?.disabled === true,
			'episode 3 to stop being offered once the show is delisted'
		);
	});

	it('captions the block with what it is actually doing', async () => {
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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
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

		app = mount(PlayPage, { target });
		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);

		card(AIRED)!.click();

		// The player's progress caption belongs to whatever ran last —
		// a provider tick from an earlier switch, say — and nothing
		// resets it on the way in here. Held open so the caption is a
		// state rather than a flicker.
		await until(
			() => (target.textContent ?? '').includes(m.detail_ep_recheck_busy()),
			'the overlay to say it is checking with allmanga'
		);
		release();
	});

	it('does not let the page-load lookup overwrite a re-ask that already answered', async () => {
		let releaseSettings!: () => void;
		const settingsHeld = new Promise<void>((r) => {
			releaseSettings = r;
		});
		let releaseModeLookup!: () => void;
		const modeLookupHeld = new Promise<void>((r) => {
			releaseModeLookup = r;
		});
		let ordinaryLookups = 0;

		server.use(
			// Settings land after the page does and flip the mode, which
			// restarts the availability lookup. That restart is what
			// puts an ordinary request in flight beside a re-ask.
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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, async ({ request }) => {
				const body = (await request.json()) as { bypass_cache?: boolean };
				if (body.bypass_cache) {
					// The user's re-ask. Authoritative and newest.
					return HttpResponse.json({
						available: true,
						episode_count: CACHED_COUNT,
						extra_episodes: ['4.5'],
						episode_count_approximate: false
					});
				}
				ordinaryLookups += 1;
				if (ordinaryLookups === 1) {
					return HttpResponse.json({
						available: true,
						episode_count: CACHED_COUNT,
						extra_episodes: [],
						episode_count_approximate: false
					});
				}
				// The mode change's lookup, held until after the re-ask
				// has landed — and unconfirmed, so its empty specials
				// list is "could not look" rather than "there are none".
				await modeLookupHeld;
				return HttpResponse.json({
					available: true,
					episode_count: 2,
					extra_episodes: [],
					episode_count_approximate: true
				});
			})
		);

		app = mount(PlayPage, { target });
		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);
		await until(() => ordinaryLookups === 1, 'the first availability lookup');

		// Mode flips; its lookup goes out and stays out.
		releaseSettings();
		await until(() => ordinaryLookups === 2, 'the mode change to restart the lookup');

		card(AIRED)!.click();
		await until(
			() => target.querySelector('li[data-ep-num="4.5"]') !== null,
			'the re-ask to land and add the special'
		);

		releaseModeLookup();

		// Bounded, and sound for the same reason as elsewhere: the
		// write happens on the microtask after the response resolves,
		// so anything that was going to clobber has by now.
		await new Promise((r) => setTimeout(r, 300));

		// Both halves of the clobber. The older answer is staler AND
		// unconfirmed, so letting it win would delete a special that
		// exists and roll the cap back to a number nobody confirmed.
		expect(target.querySelector('li[data-ep-num="4.5"]')).not.toBeNull();
		expect(card(4)?.getAttribute('title')).not.toBe(m.detail_ep_recheck_idle());
	});

	it('presents the dimmed card as something to click, not as refused', async () => {
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
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({
					available: true,
					episode_count: CACHED_COUNT,
					extra_episodes: [],
					episode_count_approximate: false
				})
			)
		);

		app = mount(PlayPage, { target });
		await until(() => card(AIRED) !== null, `the card for episode ${AIRED}`);
		await until(
			() => card(AIRED)!.getAttribute('title') === m.detail_ep_recheck_idle(),
			'the card to settle into its cap-gated resting state'
		);

		// `ep-card-unaired` is the styling for an episode that does not
		// exist yet: default cursor, no play icon. A card that re-asks
		// allmanga on click is not that, however dim it looks.
		expect(card(AIRED)!.classList.contains('ep-card-unaired')).toBe(false);
	});
});
