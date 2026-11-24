// Acceptance: a permanent answer to the live-entry read is not retried.
//
// Opening a detail page with a tracker connected reads the user's entry
// for the show. The page retries a failed read at 1, 2, 4 and 8 seconds
// so a passing failure heals itself. A tracker that rejects the request
// (the backend's `error.account.tracker_rejected`, HTTP 422) or refuses
// the bearer (401) will answer the same way however often it is asked,
// and the page used to ask it five times anyway. It now asks once and
// settles, while a 502 — the tracker failing — is still retried.
//
// The unit tests pin the plan. What they cannot see is the page: that
// the status the backend sends is the one the classifier reads, and
// that no further request leaves the app once the plan has given up.

import { describe, it, expect, vi, beforeEach, afterEach, onTestFinished } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { afterRead, watchPageFetches } from './page-fetches';
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

import type {
	AiringStatus,
	AvailabilityResponse,
	Config,
	HistoryRowByKitsu,
	KitsuAnimeRef,
	KitsuEpisode
} from '../../src/lib/api';
import { errorAnswer, availabilityAnswer } from '../answers';
const KITSU_ID = '50497';
const TITLE = 'Fresh Seasonal Show';

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

function useShowHandlers(entryReads: { count: number }, answer: () => Response) {
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json<KitsuAnimeRef>({ ...kitsuRef(KITSU_ID, TITLE, 12), status: 'current' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json<AiringStatus>({
				aired: 3,
				next_episode: 4,
				next_airing_at: null,
				upcoming: []
			})
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({
					available: true,
					episode_count: 3,
					extra_episodes: [],
					episode_count_approximate: false
				})
			)
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () =>
			HttpResponse.json<HistoryRowByKitsu | null>(null)
		),
		// The page warms the first episode's stream and reads the download
		// folder on mount; neither is what this scenario is about.
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json<string | null>('/dl')),
		http.get(`${API_BASE}/api/account/entry/anilist`, () => {
			entryReads.count += 1;
			return answer();
		})
	);
}

async function openThePage(answer: () => Response) {
	const entryReads = { count: 0 };
	useShowHandlers(entryReads, answer);
	const { sent } = watchPageFetches();
	const reads = () => sent.filter((r) => r.path === '/api/account/entry/anilist').length;
	vi.useFakeTimers({ shouldAdvanceTime: true, toFake: ['setTimeout', 'clearTimeout'] });
	onTestFinished(() => {
		vi.useRealTimers();
	});
	app = mount(DetailPage, { target });
	await until(() => listTrigger() !== null, 'the list control');
	await until(() => entryReads.count >= 1, 'the first read');
	return { entryReads, reads };
}

describe('detail route — a permanent answer to the live-entry read', () => {
	for (const [what, answer] of [
		[
			'the tracker rejecting the request',
			() =>
				errorAnswer(422, {
					kind: 'tracker_rejected',
					status: 403,
					key: 'error.account.tracker_rejected'
				})
		],
		[
			'a refused bearer',
			() => errorAnswer(401, { kind: 'invalid_token', key: 'error.stream.invalid_token' })
		]
	] as const) {
		it(`asks once for ${what}, then leaves the control disabled`, async () => {
			const { entryReads, reads } = await openThePage(answer);
			await until(() => listTrigger()?.disabled === true, 'the disabled control');
			// Every timeout the page has armed by now runs, a retry's wait
			// among them; none may have been armed for this read.
			await vi.runOnlyPendingTimersAsync();
			await afterRead();
			await vi.runOnlyPendingTimersAsync();
			await afterRead();
			expect(entryReads.count).toBe(1);
			expect(reads()).toBe(1);
			// Unknown, not absent: no Add that could overwrite an entry.
			expect(listTrigger()?.disabled).toBe(true);
		});
	}

	it('still asks again after a 502, the tracker failing', async () => {
		const { entryReads } = await openThePage(() =>
			errorAnswer(502, { kind: 'upstream', status: 503, key: 'error.network.upstream' })
		);
		await vi.runOnlyPendingTimersAsync();
		await afterRead();
		expect(entryReads.count).toBeGreaterThanOrEqual(2);
	});
});
