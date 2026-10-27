// Acceptance: a show's page follows a held progress write when it lands.
//
// A watched episode's progress write that the tracker held back can land
// long after the play — after the user has opened the show's page. The
// page read the entry when it opened and kept showing that: "Plan to
// Watch" for a show the write has since moved to Watching. When the
// write lands, the page reads the entry again, quietly, the way the
// editor reconciles its own save.

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
import { syncWatchedToTrackers } from '../../src/lib/account/push-watched';
import { toastStore } from '../../src/lib/toasts/store.svelte';

const KITSU_ID = '50498';
const OTHER_ID = '50499';

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
/** The tracker's entry for the show, as a read would find it now. */
let entry: { status: string; progress: number } = { status: 'planning', progress: 0 };
let entryReads = 0;
let progressWrites = 0;

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	setParams({ id: KITSU_ID });
	accountStore.setConnected('anilist', {
		access_token: 'test-bearer',
		refresh_token: null,
		expires_at_epoch_s: 4_000_000_000,
		user_id: 'user-1',
		username: 'tester',
		avatar_url: null
	});
	entry = { status: 'planning', progress: 0 };
	entryReads = 0;
	progressWrites = 0;
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({ ...kitsuRef(KITSU_ID, 'Planned Show', 12), status: 'finished' })
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 12, next_episode: null, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({
				available: true,
				episode_count: 12,
				extra_episodes: [],
				episode_count_approximate: false
			})
		),
		http.get(`${API_BASE}/api/history/by-kitsu/${KITSU_ID}`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/download/default-dir`, () => HttpResponse.json('/dl')),
		http.get(`${API_BASE}/api/account/entry/anilist`, () => {
			entryReads += 1;
			return HttpResponse.json(entry);
		}),
		// The progress write: held back once for a second, then taken —
		// the tracker now has the show as Watching.
		http.post(`${API_BASE}/api/account/update/anilist`, () => {
			progressWrites += 1;
			if (progressWrites === 1) {
				return HttpResponse.json(
					{
						kind: 'rate_limited',
						key: 'error.network.rate_limited',
						retry_after_secs: 1,
						refused_by: 'anilist'
					},
					{ status: 429 }
				);
			}
			entry = { status: 'watching', progress: 1 };
			return HttpResponse.json(null);
		})
	);
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

/** The list control's trigger, by the status it shows. */
const listShows = (label: string) =>
	Array.from(target.querySelectorAll('button')).some((b) => (b.textContent ?? '').includes(label));

describe("detail route — a held progress write for the show lands while it's open", () => {
	it('reads the entry again and shows where the write left it', async () => {
		const pending = syncWatchedToTrackers(KITSU_ID, 1);
		app = mount(DetailPage, { target });
		await until(() => listShows(m.detail_list_status_planning()), 'the entry as it was');
		const before = entryReads;

		await pending;
		await until(() => listShows(m.detail_list_status_watching()), 'the entry as it is now');
		expect(progressWrites).toBe(2);
		expect(entryReads).toBe(before + 1);
	});

	it("leaves the entry alone for another show's write", async () => {
		app = mount(DetailPage, { target });
		await until(() => listShows(m.detail_list_status_planning()), 'the entry as it was');
		const before = entryReads;
		await syncWatchedToTrackers(OTHER_ID, 1);
		await new Promise((r) => setTimeout(r, 100));
		expect(entryReads).toBe(before);
	});
});
