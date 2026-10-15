// Acceptance: a tracker's rate limit is waited out once and then said.
//
// With AniList connected, opening a detail page reads the user's entry
// for the show. AniList allows this machine 30 requests a minute and
// answers a 429 with `Retry-After` when they are spent; the backend
// passes that on as `error.network.rate_limited` with the wait. The
// page used to read a 429 as any other failure and retry at 1, 2, 4 and
// 8 seconds — five requests in fifteen seconds against a budget that
// resets per minute — and, when those ran out, disable the list button
// with no word about why.
//
// The unit tests pin the plan and the loop. What they cannot see is
// the page: that the loop's wait is wired to a real timer, that the
// envelope the backend sends is the one the classifier reads, and
// that the outcome reaches the control the user is looking at. So the
// assertions here are about requests that left the app and text that
// reached the DOM.

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
import { accountStore } from '../../src/lib/account/store.svelte';

const KITSU_ID = '50497';
const TITLE = 'Fresh Seasonal Show';
/** What the tracker asks the app to wait. Short, so the scenario sees
 *  the second attempt; the plan honours it as given. */
const RETRY_AFTER_S = 1;

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	// The read runs only for a connected tracker. `hydrate()` reads the
	// Electron keychain from the layout, which a page-level mount never
	// reaches — so seed the store the way hydrate would.
	accountStore.setConnected('anilist', {
		access_token: 'test-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'user-1',
		username: 'tester',
		avatar_url: null
	});
	target = document.createElement('div');
	document.body.appendChild(target);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	accountStore.setDisconnected('anilist');
});

async function until(predicate: () => boolean, what: string, timeoutMs = 8000) {
	const deadline = Date.now() + timeoutMs;
	while (Date.now() < deadline) {
		if (predicate()) return;
		await new Promise((r) => setTimeout(r, 10));
	}
	throw new Error(`timed out waiting for ${what}\n--- DOM ---\n${target.textContent}`);
}

/** The list control's trigger, found by the label it renders. */
const listTrigger = () =>
	Array.from(target.querySelectorAll('button')).find((b) =>
		(b.textContent ?? '').includes(m.detail_list_add())
	) ?? null;

/** The list control's own status line — the notice beside its trigger,
 *  not the page's other status elements (the unavailable banner, the
 *  action notice, toasts). */
const listNotice = () =>
	listTrigger()?.parentElement?.querySelector('[role="status"]')?.textContent ?? '';

function useShowHandlers(entryReads: { count: number }) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 3, next_episode: 4, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 3,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		// The page warms the first episode's stream and reads the download
		// folder on mount; neither is what this scenario is about.
		http.post(`${API_BASE}/api/play`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		// The tracker is over its limit for the whole scenario: every read
		// is refused with the wait the backend relays from `Retry-After`.
		http.get(`${API_BASE}/api/account/entry/anilist`, () => {
			entryReads.count += 1;
			return HttpResponse.json(
				{
					kind: 'rate_limited',
					retry_after_secs: RETRY_AFTER_S,
					key: 'error.network.rate_limited'
				},
				{ status: 429 }
			);
		})
	);
}

describe('detail route — the tracker answers 429 to the live-entry read', () => {
	it('asks once more after the advertised wait, then says the tracker is busy', async () => {
		const entryReads = { count: 0 };
		useShowHandlers(entryReads);

		app = mount(DetailPage, { target });

		await until(() => listTrigger() !== null, 'the list control');
		// The first refusal, the wait, the second refusal — and then the
		// page stops, with the tracker's own wait in the sentence it shows.
		const busy = m.errors_reason_busy_wait({ seconds: RETRY_AFTER_S });
		await until(() => listNotice().includes(busy), `the rate-limit notice "${busy}"`);

		expect(entryReads.count).toBe(2);
		// Unknown, not absent: the control is disabled rather than offering
		// an Add that could overwrite an entry the tracker does have.
		expect(listTrigger()?.disabled).toBe(true);

		// Nothing further leaves the app once the plan has given up.
		await new Promise((r) => setTimeout(r, 1500));
		expect(entryReads.count).toBe(2);
	});
});
