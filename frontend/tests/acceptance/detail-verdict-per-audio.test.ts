// Acceptance: the detail page's availability verdict is about one audio.
//
// The page asks the provider about the show in the chosen audio, and
// the viewer can switch the audio on the page. A show delisted in sub
// can be listed in dub: once the viewer picks dub, the sub answer says
// nothing about it. While the dub answer is not in — still out, failed,
// or back without a verdict — the page behaves as it does whenever
// availability is not yet known: no tile is disabled for the show
// being delisted, and a settled-but-unknown answer leaves Play on
// screen instead of the "not in the catalogue" notice.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams } from './page-state.svelte';
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
import { __resetApiBaseForTests } from '../../src/lib/api';

const KITSU_ID = '40684';
const TITLE = 'Two Audio Show';

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

type DubAnswer = 'failed' | 'unsettled' | 'pending';

/** The page in sub, where the show is delisted; the dub question ends
 *  as `dub` says. */
function useHandlers(dub: DubAnswer) {
	const asked = { dub: false };
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig('sub'))),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			const body = (await request.json()) as { mode: string };
			if (body.mode === 'sub') {
				return HttpResponse.json({ available: false, episode_count: null, extra_episodes: [] });
			}
			asked.dub = true;
			if (dub === 'pending') return new Promise<never>(() => {});
			if (dub === 'failed') return new HttpResponse(null, { status: 500 });
			return HttpResponse.json({
				available: false,
				episode_count: null,
				extra_episodes: [],
				unsettled: 'unanswered'
			});
		}),
		http.post(`${API_BASE}/api/availability/batch`, () =>
			HttpResponse.json({ cached: {}, playable_episode_counts: {} })
		),
		http.post(`${API_BASE}/api/availability/warm`, () => new HttpResponse(null, { status: 202 })),
		http.put(`${API_BASE}/api/settings`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/history/by-kitsu/:id`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({}))
	);
	return asked;
}

const notice = () => target.querySelector('p.unavailable[role="status"]');
const tiles = () => Array.from(target.querySelectorAll<HTMLButtonElement>('button.ep-tile'));
const play = () => target.querySelector('div.actions button.btn-glass');

/** Delisted in sub, then switched to dub, with the dub question out. */
async function switchToDub(asked: { dub: boolean }) {
	app = mount(DetailPage, { target });
	await until(() => notice() !== null, 'the sub notice');
	await until(() => tiles().length > 0, 'the episode tiles');
	expect(tiles()[0].getAttribute('aria-disabled')).toBe('true');
	const dubButton = Array.from(target.querySelectorAll('button.seg-btn')).find(
		(b) => b.textContent?.trim() === 'DUB'
	) as HTMLButtonElement;
	dubButton.click();
	await until(() => asked.dub, 'the dub question');
	// Whatever the dub question comes back with is read by now.
	await new Promise((r) => setTimeout(r, 50));
}

describe('detail — a verdict for sub does not gate dub', () => {
	for (const dub of ['failed', 'unsettled', 'pending'] as const) {
		it(`delisted in sub, with the dub answer ${dub}: the tiles are not held delisted`, async () => {
			const asked = useHandlers(dub);
			await switchToDub(asked);
			await until(
				() => tiles().length > 0 && !tiles().some((t) => t.classList.contains('ep-tile-disabled')),
				'tiles not held delisted'
			);
			expect(notice()).toBeNull();
			if (dub !== 'pending') expect(play()).not.toBeNull();
		});
	}
});
