// Acceptance: the notice that replaces Play names the other audio
// when the availability cache has the title in it.
//
// Kitsu 40684 is in the catalogue with subtitles and has nothing
// dubbed. In dub, the detail page hid Play behind "Not in the
// streaming catalogue", which is false: the title is there, in the
// other audio. These cases mount the detail page in dub with the
// probe answering unavailable, answer the cache's question about the
// other audio several ways, and read the notice. They also pin that
// the other audio is learned from the cache-only batch route, with no
// probe and no warm.

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

const KITSU_ID = '40684';
const TITLE = 'Ongoing Show';

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

interface Asked {
	batch: { kitsu_ids: string[]; mode: string }[];
	probeModes: string[];
	warms: number;
}

/** The page in `mode`; the probe answers unavailable for it, and the
 *  cache answers `other` for the other audio. */
function useHandlers(mode: 'sub' | 'dub', other: CacheAnswer | 'fails'): Asked {
	const asked: Asked = { batch: [], probeModes: [], warms: 0 };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(mode))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			asked.probeModes.push(((await request.json()) as { mode: string }).mode);
			return HttpResponse.json({ available: false, episode_count: null, extra_episodes: [] });
		}),
		http.post(`${API_BASE}/api/availability/batch`, async ({ request }) => {
			const body = (await request.json()) as { kitsu_ids: string[]; mode: string };
			asked.batch.push(body);
			if (body.mode === mode) return HttpResponse.json({ cached: {}, playable_episode_counts: {} });
			if (other === 'fails') return new HttpResponse(null, { status: 500 });
			return HttpResponse.json(other);
		}),
		http.post(`${API_BASE}/api/availability/warm`, () => {
			asked.warms += 1;
			return new HttpResponse(null, { status: 202 });
		}),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	return asked;
}

const notice = () => target.querySelector('p.unavailable[role="status"]')?.textContent ?? null;

const HAS = { cached: { [KITSU_ID]: true }, playable_episode_counts: { [KITSU_ID]: 4 } };

const CASES: [string, 'sub' | 'dub', CacheAnswer | 'fails', () => string][] = [
	['dub, and the cache has it subtitled', 'dub', HAS, () => m.detail_unavailable_in_dub_message()],
	['sub, and the cache has it dubbed', 'sub', HAS, () => m.detail_unavailable_in_sub_message()],
	[
		'dub, and the cache knows the other audio has nothing',
		'dub',
		{ cached: { [KITSU_ID]: false }, playable_episode_counts: {} },
		() => m.detail_unavailable_message()
	],
	[
		'dub, and the cache holds no answer for the other audio',
		'dub',
		{ cached: {}, playable_episode_counts: {} },
		() => m.detail_unavailable_message()
	],
	[
		'dub, and the cache has a count-less row for the other audio',
		'dub',
		{ cached: { [KITSU_ID]: true }, playable_episode_counts: {} },
		() => m.detail_unavailable_in_dub_message()
	],
	['dub, and the cache lookup fails', 'dub', 'fails', () => m.detail_unavailable_message()]
];

describe('detail — the unavailable notice and the other audio', () => {
	for (const [when, mode, other, expected] of CASES) {
		it(`in ${when}`, async () => {
			const asked = useHandlers(mode, other);
			const otherMode = mode === 'dub' ? 'sub' : 'dub';
			app = mount(DetailPage, { target });
			await until(() => notice() !== null, 'the unavailable notice');
			await until(
				() => asked.batch.some((b) => b.mode === otherMode),
				'the cache lookup for the other audio'
			);
			await until(() => (notice() ?? '').includes(expected()), 'the notice copy');
			expect(asked.batch).toContainEqual({ kitsu_ids: [KITSU_ID], mode: otherMode });
			expect(asked.probeModes, 'the provider is never asked about the other audio').not.toContain(
				otherMode
			);
			expect(asked.warms, 'nothing warms the cache for the notice').toBe(0);
		});
	}
});
