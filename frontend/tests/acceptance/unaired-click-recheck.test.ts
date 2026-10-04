// Acceptance: clicking an episode the page calls unaired checks
// whether that is still true.
//
// The schedule and the episode list are both cached, so a page can
// call an episode unaired well after it came out. The click is the
// user asking; it has to reach past both caches, and when the fresh
// answer says the episode is out, carry on as any aired episode would:
// play it, or ask the provider when its catalogued count falls short.
// When the fresh answer agrees, the user hears so, and a second click
// within a few minutes does not ask again.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { appConfig, kitsuRef } from './home-handlers';

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
import PlayPage from '../../src/routes/play/[id]/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import { m } from '../../src/lib/paraglide/messages';

function episode(n: number, airdate: string | null) {
	return {
		id: `ep-${n}`,
		canonical_title: `Episode ${n}`,
		season_number: 1,
		number: n,
		relative_number: null,
		length: 24,
		synopsis: null,
		airdate,
		thumbnail: null
	};
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let refreshes: string[];
let played: { episode?: string; prefetch?: boolean }[];

/**
 * A weekly show as the caches remember it — two episodes out, the
 * third due — and as the services answer when asked past the cache:
 * the third has aired. `airsOnRefresh: false` keeps the fresh answer
 * the same as the cached one.
 */
function serve(kitsuId: string, opts: { airsOnRefresh: boolean }) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${kitsuId}`, () =>
			HttpResponse.json({ ...kitsuRef(kitsuId, 'Weekly Show', 12), status: 'current' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${kitsuId}`, ({ request }) => {
			const fresh = new URL(request.url).searchParams.get('refresh') === 'true';
			if (fresh) refreshes.push('airing');
			const aired = fresh && opts.airsOnRefresh ? 3 : 2;
			return HttpResponse.json({
				aired,
				next_episode: aired + 1,
				next_airing_at: null,
				upcoming: []
			});
		}),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ request }) => {
			const fresh = new URL(request.url).searchParams.get('refresh') === 'true';
			if (fresh) refreshes.push('episodes');
			const third = fresh && opts.airsOnRefresh ? '2026-01-15' : null;
			return HttpResponse.json([
				episode(1, '2026-01-01'),
				episode(2, '2026-01-08'),
				episode(3, third)
			]);
		}),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 3,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.post(`${API_BASE}/api/play`, async ({ request }) => {
			played.push((await request.json()) as { episode?: string; prefetch?: boolean });
			return HttpResponse.json({
				id: 'session-1',
				kind: 'hls',
				has_subtitles: false,
				quality: '1080',
				mode: 'sub'
			});
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null))
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	target = document.createElement('div');
	document.body.appendChild(target);
	refreshes = [];
	played = [];
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
const playedEpisode = (n: number) => played.some((p) => p.episode === String(n) && !p.prefetch);

describe('detail route — clicking an episode the page calls unaired', () => {
	it('reaches past both caches and plays the episode once it has aired', async () => {
		// Each scenario its own show: the click's rate limit is per show
		// and lasts the app session.
		setParams({ id: '7001' });
		serve('7001', { airsOnRefresh: true });
		app = mount(DetailPage, { target });
		await until(() => tile(3)?.classList.contains('ep-tile-unaired') === true, 'tile 3 unaired');

		tile(3)!.click();

		await until(() => refreshes.includes('airing'), 'the schedule refreshed past its cache');
		await until(() => refreshes.includes('episodes'), 'the episode page refreshed');
		await until(() => tile(3)?.classList.contains('ep-tile-unaired') === false, 'tile 3 aired');
		await until(() => playedEpisode(3), 'episode 3 to play');
	});

	it('says it still has not aired, and does not ask again straight away', async () => {
		setParams({ id: '7002' });
		serve('7002', { airsOnRefresh: false });
		app = mount(DetailPage, { target });
		await until(() => tile(3)?.classList.contains('ep-tile-unaired') === true, 'tile 3 unaired');

		tile(3)!.click();
		await until(() => refreshes.length === 2, 'one refresh of each');
		await until(
			() => toastStore.items.some((t) => t.message === m.detail_ep_unaired_still()),
			'the still-unaired message'
		);
		await until(() => tile(3)?.getAttribute('aria-disabled') === 'false', 'the page released');

		tile(3)!.click();
		await new Promise((r) => setTimeout(r, 100));
		expect(refreshes).toHaveLength(2);
		expect(playedEpisode(3)).toBe(false);
	});
});

describe('play route — clicking an episode the strip calls unaired', () => {
	it('reaches past both caches and switches to the episode once it has aired', async () => {
		setParams({ id: '7003' });
		setUrl('/play/7003', { episode: '1' });
		serve('7003', { airsOnRefresh: true });
		app = mount(PlayPage, { target });
		await until(() => tile(3)?.classList.contains('ep-card-unaired') === true, 'card 3 unaired');

		tile(3)!.click();

		await until(() => refreshes.includes('airing'), 'the schedule refreshed past its cache');
		await until(() => refreshes.includes('episodes'), 'the episode page refreshed');
		await until(() => playedEpisode(3), 'the player to switch to episode 3');
	});
});
