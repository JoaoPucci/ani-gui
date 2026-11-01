// Acceptance: under reduced motion, nothing on the player page moves.
//
// The player's own chrome — the menu under its "more" button, the
// loader that covers the frame while the next episode is on its way,
// a jump along the episode strip — stands still. The CSS half (the
// spinner, the controls' fade) is asserted in a real Chromium by the
// Electron e2e; see ./motion.ts.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { recordMotion, setReducedMotion, type Motion } from './motion';
import { playerVideoInSlot } from './player-video';

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
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';
const TITLE = 'Ongoing Show';
const EPISODES = 12;

/** Never answers: an episode switch stays on its way, and its loader
 *  with it. */
class SilentEventSource {
	addEventListener() {}
	close() {}
}
type GlobalLike = { EventSource?: unknown };
const g = globalThis as unknown as GlobalLike;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let motion: Motion;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	g.EventSource = SilentEventSource;
	setParams({ id: KITSU_ID });
	motion = recordMotion();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	motion.stop();
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

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

function mountPlayer() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, EPISODES), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: EPISODES, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
			HttpResponse.json(kitsuEpisodes(EPISODES))
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: EPISODES,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.post(`${API_BASE}/api/play/cache/evict`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
	setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
	app = mount(PlayPage, { target });
}

const tile = (n: number) =>
	target.querySelector<HTMLButtonElement>(`li[data-ep-num="${n}"] button`);

afterEach(() => setReducedMotion(false));

describe.each([
	['under reduced motion', true],
	['with motion on', false]
] as const)('the player %s', (_mode, reduced) => {
	beforeEach(() => setReducedMotion(reduced));

	it('opens its menu', async () => {
		mountPlayer();
		await until(() => playerVideoInSlot(), 'the video in its slot');
		await until(
			() => target.querySelector('[aria-controls="np-more-menu"]') !== null,
			'the more button'
		);
		target.querySelector<HTMLButtonElement>('[aria-controls="np-more-menu"]')!.click();
		await until(() => target.querySelector('#np-more-menu') !== null, 'the menu');

		if (reduced) expect(motion.animations).toEqual([]);
		else expect(motion.animations.some((a) => a.includes('more-menu'))).toBe(true);
	});

	it('shows the loader while the next episode is on its way', async () => {
		mountPlayer();
		await until(() => (target.textContent ?? '').includes(TITLE), 'the show detail');
		await until(() => tile(3) !== null, 'the episode strip');
		tile(3)!.click();
		await until(() => motion.lottie().length > 0, 'the loader animation to load');

		const played = motion.lottie().includes('play') || motion.lottie().includes('load:autoplay');
		expect(played).toBe(!reduced);
		if (reduced) expect(motion.animations).toEqual([]);
	});

	it('scrolls the episode strip to a jumped-to episode', async () => {
		mountPlayer();
		await until(() => target.querySelector('form.ep-jump input') !== null, 'the jump field');
		const input = target.querySelector<HTMLInputElement>('form.ep-jump input')!;
		input.value = '4';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		target
			.querySelector<HTMLFormElement>('form.ep-jump')!
			.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
		await until(
			() => target.querySelector('li.ep-highlight[data-ep-num="4"]') !== null,
			'the episode to be picked out'
		);
		await new Promise((r) => setTimeout(r, 100));

		expect(motion.glides).toEqual(reduced ? [] : [expect.stringContaining('scrollIntoView')]);
	});
});
