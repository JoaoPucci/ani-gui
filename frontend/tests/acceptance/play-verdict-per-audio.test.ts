// Acceptance: the player's availability verdict is about one audio.
//
// The player asks the provider about the show in the chosen audio,
// and settings land after it has started asking: a viewer who chose
// dub has the default sub question asked first, then the dub one. The
// sub answer says nothing about dub. While the dub answer is not in —
// still out, failed, or back without a verdict — the page knows
// nothing about dub, and behaves as it does whenever availability is
// not yet known: no episode is refused for the show being delisted or
// for sitting past a count, and the player-error overlay keeps its
// Reload.
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
const TITLE = 'Two Audio Show';
const NOTHING_CACHED = { cached: {}, playable_episode_counts: {} };

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

/** How the dub question ends. */
type DubAnswer = 'failed' | 'unsettled' | 'pending';

interface Seen {
	subAnswered: boolean;
	dubAsked: boolean;
	evictions: number;
}

/**
 * The sub answer is `sub`; settings, which choose dub, land only after
 * it. The dub question then ends as `dub` says.
 */
function useHandlers(sub: { available: boolean; episode_count: number | null }, dub: DubAnswer) {
	const seen: Seen = { subAnswered: false, dubAsked: false, evictions: 0 };
	let releaseSettings = () => {};
	const settingsHeld = new Promise<void>((r) => (releaseSettings = r));
	server.use(
		http.get(`${API_BASE}/api/settings`, async () => {
			await settingsHeld;
			return HttpResponse.json(appConfig('dub'));
		}),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			const body = (await request.json()) as { mode: string };
			if (body.mode === 'sub') {
				seen.subAnswered = true;
				// Settings land once the sub answer has been read.
				setTimeout(releaseSettings, 50);
				return HttpResponse.json({ ...sub, extra_episodes: [] });
			}
			seen.dubAsked = true;
			if (dub === 'pending') return new Promise<never>(() => {});
			if (dub === 'failed') return new HttpResponse(null, { status: 500 });
			return HttpResponse.json({
				available: false,
				episode_count: null,
				extra_episodes: [],
				unsettled: 'unanswered'
			});
		}),
		http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json(NOTHING_CACHED)),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		// The fresh resolve a Reload that goes through asks for.
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => {
			seen.evictions += 1;
			return new HttpResponse(null, { status: 204 });
		}),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	return seen;
}

/** Mount on episode `episode`'s session, wait for the sub answer and
 *  for the dub question, then fail the stream. */
async function failStreamAfterAudioChange(seen: Seen, episode: number) {
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: String(episode), kind: 'mp4' });
	app = mount(PlayPage, { target });
	await until(() => playerVideoInSlot(), 'the video in its slot');
	await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
	await until(() => seen.subAnswered, 'the sub answer');
	await until(() => seen.dubAsked, 'the dub question');
	// Whatever the dub question comes back with is read by now.
	await new Promise((r) => setTimeout(r, 50));
	const video = playerVideo();
	Object.defineProperty(video, 'error', { value: { code: DECODE }, configurable: true });
	video.dispatchEvent(new Event('error'));
	await until(() => target.querySelector('.player-error') !== null, 'the error overlay');
}

const DECODE = 3;
const overlay = () => target.querySelector('.player-error');
const reloadButton = () =>
	overlay()?.querySelector<HTMLButtonElement>('button.player-error-reload') ?? null;
const body = () => overlay()?.querySelector('.player-error-body')?.textContent?.trim() ?? null;
const tile = (n: number) =>
	Array.from(target.querySelectorAll<HTMLButtonElement>('button.ep-card')).find((b) =>
		(b.textContent ?? '').includes(m.play_episode_card_num({ episode: String(n) }))
	) ?? null;

/** The overlay offers the Reload, and the Reload goes through. */
async function expectReloadWorks(seen: Seen) {
	expect(body()).toBe(m.play_error_stream_body());
	const reload = reloadButton();
	expect(reload?.textContent?.trim()).toBe(m.play_error_stream_reload());
	await until(() => !reloadButton()?.disabled, 'Reload enabled');
	reloadButton()?.click();
	await until(() => seen.evictions === 1, 'the Reload recovery');
}

describe('play route — a verdict for sub does not gate dub', () => {
	for (const dub of ['failed', 'unsettled', 'pending'] as const) {
		it(`a show delisted in sub, with the dub answer ${dub}: Reload stays`, async () => {
			const seen = useHandlers({ available: false, episode_count: null }, dub);
			await failStreamAfterAudioChange(seen, 1);
			// The strip does not hold the show delisted in dub either.
			expect(tile(2)?.disabled).toBe(false);
			await expectReloadWorks(seen);
		});
	}

	it('an episode past the sub count, with the dub answer failed: Reload stays', async () => {
		const seen = useHandlers({ available: true, episode_count: 8 }, 'failed');
		await failStreamAfterAudioChange(seen, 10);
		await expectReloadWorks(seen);
	});
});
