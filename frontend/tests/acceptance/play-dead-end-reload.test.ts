// Acceptance: the player offers Reload only where a reload can work.
//
// Reload re-resolves the episode on screen through the switch every
// play path on the player uses, and that switch refuses a show the
// provider no longer lists in the chosen audio, an episode the
// schedule calls unaired, and one past the provider's playable count.
// A Reload on any of those is a button that can never succeed: the
// overlay instead says why, in the words the rest of the app uses for
// that state, and links back to the show's page, where the audio
// switch is. A failure the switch would take keeps its Reload.
//
// The stream is an mp4 session — happy-dom has no MediaSource — and
// the failure is simulated the way Chromium reports it: a MediaError
// code on the element plus an `error` event.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { m } from '../../src/lib/paraglide/messages';
import { toastStore } from '../../src/lib/toasts/store.svelte';

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
import { playerVideo, playerVideoInSlot } from './player-video';

const KITSU_ID = '42';
const TITLE = 'Delisted Show';

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

type CacheAnswer = {
	cached: Record<string, boolean>;
	playable_episode_counts: Record<string, number>;
};
const NOTHING_CACHED: CacheAnswer = { cached: {}, playable_episode_counts: {} };

interface Scenario {
	mode?: 'sub' | 'dub';
	/** The provider's answer for the chosen audio. */
	availability: { available: boolean; episode_count: number | null };
	/** The cache's answer for the other audio. */
	other?: CacheAnswer;
	/** The schedule's aired count; `null` for a finished show. */
	aired?: number | null;
}

interface Seen {
	availabilityAnswered: boolean;
	evictions: number;
}

function useHandlers(s: Scenario): Seen {
	const seen: Seen = { availabilityAnswered: false, evictions: 0 };
	const mode = s.mode ?? 'sub';
	const airing = s.aired ?? null;
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(mode))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({
				...kitsuRef(KITSU_ID, TITLE, 12),
				status: airing === null ? 'finished' : 'current'
			})
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json(
				airing === null
					? null
					: { aired: airing, next_episode: airing + 1, next_airing_at: null, upcoming: [] }
			)
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () => {
			seen.availabilityAnswered = true;
			return HttpResponse.json({ ...s.availability, extra_episodes: [] });
		}),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const body = (await request.json()) as { mode: string };
			return HttpResponse.json(body.mode === mode ? NOTHING_CACHED : (s.other ?? NOTHING_CACHED));
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => {
			seen.evictions += 1;
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	return seen;
}

/** Mount on episode `episode`'s session, wait for what the page learns
 *  about the show, then fail the stream with MediaError `code`. */
async function failStream(seen: Seen, episode: number, code: number) {
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: String(episode), kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => seen.availabilityAnswered, 'the availability answer');
	// The answer is written back once its response is read.
	await new Promise((r) => setTimeout(r, 50));
	const video = playerVideo();
	Object.defineProperty(video, 'error', { value: { code }, configurable: true });
	video.dispatchEvent(new Event('error'));
	await until(() => target.querySelector('.player-error') !== null, 'the error overlay');
}

const overlay = () => target.querySelector('.player-error');
const reloadButton = () => overlay()?.querySelector('button.player-error-reload') ?? null;
const body = () => overlay()?.querySelector('.player-error-body')?.textContent?.trim() ?? null;
const backLink = () =>
	overlay()?.querySelector<HTMLAnchorElement>(`a[href$="/anime/${KITSU_ID}"]`) ?? null;

/** The overlay settles on what the cache says about the other audio,
 *  so the body is read once it has had the chance to. */
async function bodyBecomes(expected: string) {
	await until(() => body() === expected, `the overlay body "${expected}"`);
}

function expectDeadEnd() {
	expect(reloadButton()).toBeNull();
	expect(backLink()?.textContent?.trim()).toBe(m.play_error_stream_back_to_show());
}

const DECODE = 3;
const NETWORK = 2;

describe('play route — the player-error overlay offers Reload only where it can work', () => {
	it('a show the provider no longer lists: the catalogue notice, no Reload', async () => {
		const seen = useHandlers({ availability: { available: false, episode_count: null } });
		await failStream(seen, 1, DECODE);
		await bodyBecomes(m.detail_unavailable_message());
		expectDeadEnd();
	});

	it('a show delisted in dub that the cache has subtitled names the other audio', async () => {
		const seen = useHandlers({
			mode: 'dub',
			availability: { available: false, episode_count: null },
			other: { cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 12 } }
		});
		await failStream(seen, 1, DECODE);
		await bodyBecomes(m.play_error_stream_unavailable_in_dub());
		expectDeadEnd();
	});

	it('a show delisted in sub that the cache has dubbed names the other audio', async () => {
		const seen = useHandlers({
			mode: 'sub',
			availability: { available: false, episode_count: null },
			other: { cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 12 } }
		});
		await failStream(seen, 1, DECODE);
		await bodyBecomes(m.play_error_stream_unavailable_in_sub());
		expectDeadEnd();
	});

	it('an episode past the playable count: the episode verdict, no Reload', async () => {
		const seen = useHandlers({ availability: { available: true, episode_count: 8 } });
		await failStream(seen, 10, DECODE);
		await bodyBecomes(m.play_play_failure_episode_unavailable());
		expectDeadEnd();
	});

	it('past the count, with the other audio cached as covering it, suggests that audio', async () => {
		const seen = useHandlers({
			availability: { available: true, episode_count: 8 },
			other: { cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 12 } }
		});
		await failStream(seen, 10, DECODE);
		await bodyBecomes(m.play_play_failure_episode_unavailable_try_other_audio());
		expectDeadEnd();
	});

	it('an episode the schedule calls unaired: says so, no Reload', async () => {
		const seen = useHandlers({ availability: { available: true, episode_count: 12 }, aired: 5 });
		await failStream(seen, 7, DECODE);
		await bodyBecomes(m.detail_ep_unaired_tooltip());
		expectDeadEnd();
	});

	it('a network failure on a delisted show is not sent on a recovery that cannot land', async () => {
		const seen = useHandlers({ availability: { available: false, episode_count: null } });
		const toastsBefore = toastStore.items.length;
		await failStream(seen, 1, NETWORK);
		await bodyBecomes(m.detail_unavailable_message());
		expectDeadEnd();
		// No eviction and no "recovering" notice: the recovery would end
		// in the switch refusing the episode.
		expect(seen.evictions).toBe(0);
		expect(toastStore.items.length).toBe(toastsBefore);
	});

	it('a failure on an episode the switch would take keeps its Reload', async () => {
		const seen = useHandlers({ availability: { available: true, episode_count: 12 } });
		await failStream(seen, 1, DECODE);
		expect(reloadButton()?.textContent?.trim()).toBe(m.play_error_stream_reload());
		expect(body()).toBe(m.play_error_stream_body());
	});
});
