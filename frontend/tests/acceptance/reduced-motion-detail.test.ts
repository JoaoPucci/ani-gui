// Acceptance: under reduced motion, nothing on a show's page moves.
//
// With motion on, the same steps do move, so the scenarios cannot pass
// by watching nothing. The banner holds its place as the page scrolls,
// a jump to an episode lands on it instead of gliding, and the loader
// that covers the page while a play starts holds still. The CSS half is
// asserted in a real Chromium by the Electron e2e; see ./motion.ts.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import {
	recordMotion,
	restoreWindowScroll,
	scrollWindowTo,
	setReducedMotion,
	type Motion
} from './motion';

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

import { goto } from '$app/navigation';
import DetailPage from '../../src/routes/anime/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { __resetPlayCacheForTests } from '../../src/lib/play/play-cache';

const KITSU_ID = '42';
const EPISODES = 30;

/** Never answers: a play started here stays on its way, and the
 *  loader with it. */
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
	g.EventSource = SilentEventSource;
	localStorage.clear();
	setParams({ id: KITSU_ID });
	setUrl(`/anime/${KITSU_ID}`);
	// The page's own navigations (`?page=`, `?ep=`) land, as they do in
	// the app, so a jump to another page of episodes renders it.
	vi.mocked(goto).mockImplementation(async (to) => {
		const url = new URL(String(to), 'http://127.0.0.1:31337');
		setUrl(url.pathname, Object.fromEntries(url.searchParams));
	});
	motion = recordMotion();
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(async () => {
	if (app) unmount(app);
	app = null;
	delete g.EventSource;
	target.remove();
	motion.stop();
	restoreWindowScroll();
	// After the page is gone, so no mounted page answers the change.
	await setReducedMotion(false);
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

function mountShow() {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({
				...kitsuRef(KITSU_ID, 'Long Show', EPISODES),
				status: 'finished',
				poster_image: { large: 'https://media.kitsu.app/anime/42/poster_image/large.jpg' }
			})
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
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/tmp'))
	);
	app = mount(DetailPage, { target });
}

const tile = (n: number) =>
	target.querySelector<HTMLButtonElement>(`li[data-ep-num="${n}"] button`);

describe.each([
	['under reduced motion', true],
	['with motion on', false]
] as const)("a show's page %s", (_mode, reduced) => {
	beforeEach(() => setReducedMotion(reduced));

	it('scrolls past the banner', async () => {
		mountShow();
		await until(() => target.querySelector('section.hero img.hero-img') !== null, 'the banner');
		const banner = () => target.querySelector<HTMLElement>('section.hero img.hero-img')!;
		expect(banner().style.transform).toContain('translate3d(0, 0px, 0)');

		await scrollWindowTo(200);
		// A quarter of the scroll with motion on; still under reduced motion.
		expect(banner().style.transform).toContain(
			reduced ? 'translate3d(0, 0px, 0)' : 'translate3d(0, 50px, 0)'
		);
	});

	it('scrolls to a jumped-to episode', async () => {
		mountShow();
		await until(() => target.querySelector('form.ep-jump input') !== null, 'the jump field');
		const input = target.querySelector<HTMLInputElement>('form.ep-jump input')!;
		input.value = '9';
		input.dispatchEvent(new Event('input', { bubbles: true }));
		target
			.querySelector<HTMLFormElement>('form.ep-jump')!
			.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
		await until(
			() => target.querySelector('li.ep-highlight[data-ep-num="9"]') !== null,
			'the episode to be picked out'
		);
		// The scroll waits a frame for the tile to be laid out.
		await new Promise((r) => setTimeout(r, 100));

		expect(motion.glides).toEqual(reduced ? [] : [expect.stringContaining('scrollIntoView')]);
	});

	it('shows the loader while a play is on its way', async () => {
		mountShow();
		await until(() => tile(1) !== null, 'the first tile');
		tile(1)!.click();
		await until(() => target.querySelector('[role="status"] .anim') !== null, 'the loader');
		await until(() => motion.lottie().length > 0, 'the loader animation to load');

		const played = motion.lottie().includes('play') || motion.lottie().includes('load:autoplay');
		expect(played).toBe(!reduced);
		if (reduced) expect(motion.animations).toEqual([]);
	});
});
