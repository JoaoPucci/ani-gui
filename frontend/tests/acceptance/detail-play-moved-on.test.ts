// Acceptance: a play clicked on the detail page opens no player over a
// navigation the viewer started while it resolved.
//
// The click awaits its session and then calls goto for the player.
// The page stays on screen until a navigation the viewer started —
// a link, Back — lands, and a goto in that window cancels the
// navigation they asked for and puts them in the player instead.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';
import { forgetNavigationListeners, startNavigation } from './navigation';

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
const TITLE = 'Finished Show';
/** An episode no warm asks for, so only the click resolves it. */
const CLICKED = 7;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	__resetPlayCacheForTests();
	window.localStorage.clear();
	vi.mocked(goto).mockClear();
	forgetNavigationListeners();
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

const tile = (n: number) =>
	(target.querySelector(`li[data-ep-num="${n}"] button`) as HTMLButtonElement | null) ?? null;
const overlay = () => target.querySelector('[role="status"].backdrop');

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

/** Serves the show; the clicked episode's play waits for `held`. */
function serve(held: Promise<void>, asked: () => void) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play`, async ({ request }) => {
			const body = (await request.json()) as { episode?: string; prefetch?: boolean };
			if (body.episode === String(CLICKED) && !body.prefetch) {
				asked();
				await held;
			}
			return HttpResponse.json({
				id: `session-${body.episode}`,
				kind: 'hls',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl'))
	);
}

describe('detail route — a play that resolves after a navigation started', () => {
	it('opens no player, and lets the page go', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let asked = false;
		serve(held, () => (asked = true));
		app = mount(DetailPage, { target });
		await until(() => tile(CLICKED) !== null, `the tile for episode ${CLICKED}`);

		tile(CLICKED)!.click();
		await until(() => asked, 'the click to ask for its session');
		startNavigation();
		release();
		await new Promise((r) => setTimeout(r, 300));

		expect(goto).not.toHaveBeenCalled();
		// The page is still on screen until that navigation lands.
		expect(overlay()).toBeNull();
	});

	it('opens the player while the viewer stays', async () => {
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		let asked = false;
		serve(held, () => (asked = true));
		app = mount(DetailPage, { target });
		await until(() => tile(CLICKED) !== null, `the tile for episode ${CLICKED}`);

		tile(CLICKED)!.click();
		await until(() => asked, 'the click to ask for its session');
		release();
		await until(() => vi.mocked(goto).mock.calls.length > 0, 'the navigation to the player');
	});
});
