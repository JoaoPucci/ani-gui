// Acceptance: the Watch Later row on Home follows a held tracker write.
//
// The user adds a show to Plan to Watch — or removes one — while
// AniList's allowance is spent, so the save is held with a countdown.
// They go Home before it lands. When it lands, the row on screen has to
// show the list as it now is: the show joins the row, or leaves it, in
// place, without a reload — moving the way a card leaves the Continue
// Watching strip. A write dropped before it went out changes nothing,
// and a row mounted after the write landed simply reads the new list.
//
// The write goes through the editor's own save path (`syncSetEntry`,
// `syncRemoveEntry`); the row is the real home route over a fake
// backend whose cached list changes when the write lands, as the
// backend's write-through changes it.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page } from './page-state.svelte';
import { homeHandlers, kitsuRef, type KitsuRefFixture } from './home-handlers';

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
// The strip motion, spied on: which cards the row moves, and how, is
// what the gate decided at fire time.
const { scaled } = vi.hoisted(() => ({ scaled: [] as { id: string; duration: number }[] }));
vi.mock('../../src/lib/transitions/card-motion', async (importOriginal) => {
	const real = await importOriginal<typeof import('../../src/lib/transitions/card-motion')>();
	return {
		...real,
		cardScale: (node: Element) => {
			const config = real.cardScale(node);
			scaled.push({
				id: (node as HTMLElement).dataset.animeId ?? '',
				duration: config.duration ?? 0
			});
			return config;
		}
	};
});

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import { syncRemoveEntry, syncSetEntry } from '../../src/lib/account/set-entry';
import { toastStore } from '../../src/lib/toasts/store.svelte';
import type { ListEntry } from '../../src/lib/account/types';
import { m } from '../../src/lib/paraglide/messages';

const ALREADY = { malId: 11, title: 'Already Planned', updated: 1_700_000_000 };
const NEW = { malId: 12, title: 'Just Planned', updated: 1_700_000_500 };

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
/** What the backend's cached Plan-to-Watch holds right now. */
let planned: (typeof ALREADY)[] = [];
let writes = 0;

const as = (user_id: string) => ({
	access_token: `bearer-${user_id}`,
	refresh_token: null,
	expires_at_epoch_s: 4_000_000_000,
	user_id,
	username: 'tester',
	avatar_url: null
});

function listEntry(p: typeof ALREADY): ListEntry {
	return {
		provider: 'anilist',
		media_id: p.malId,
		mal_id: p.malId,
		status: 'planning',
		progress_episodes: 0,
		score_0_to_100: null,
		updated_at_epoch_s: p.updated,
		title: p.title
	};
}

const refusal = () =>
	HttpResponse.json(
		{
			kind: 'rate_limited',
			key: 'error.network.rate_limited',
			retry_after_secs: 1,
			refused_by: 'anilist'
		},
		{ status: 429 }
	);

/** The home route's requests, a cached list that is `planned`, and a
 *  tracker that holds the first write back for a second and takes the
 *  next, changing the cached list as the backend's write-through does. */
function backend() {
	server.use(
		...homeHandlers({}, [
			http.get(`${API_BASE}/api/account/list/anilist/cached`, () =>
				HttpResponse.json(planned.map(listEntry))
			),
			http.post(`${API_BASE}/api/kitsu/by-mal-ids`, async ({ request }) => {
				const { mal_ids } = (await request.json()) as { mal_ids: number[] };
				const all = [ALREADY, NEW];
				return HttpResponse.json(
					mal_ids
						.map((id) => all.find((p) => p.malId === id))
						.filter((p): p is typeof ALREADY => p !== undefined)
						.map((p) => kitsuRef(String(p.malId), p.title, 12) as KitsuRefFixture)
				);
			}),
			http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} })),
			http.get(`${API_BASE}/api/account/entry/anilist`, ({ request }) => {
				const id = Number(new URL(request.url).searchParams.get('kitsu_id'));
				return HttpResponse.json(
					planned.some((p) => p.malId === id) ? { status: 'planning', progress: 0 } : null
				);
			}),
			http.post(`${API_BASE}/api/account/set/anilist`, () => {
				writes += 1;
				if (writes === 1) return refusal();
				planned = [...planned.filter((p) => p.malId !== NEW.malId), NEW];
				return HttpResponse.json(listEntry(NEW));
			}),
			http.delete(`${API_BASE}/api/account/entry/anilist`, ({ request }) => {
				writes += 1;
				if (writes === 1) return refusal();
				const id = Number(new URL(request.url).searchParams.get('kitsu_id'));
				planned = planned.filter((p) => p.malId !== id);
				return new HttpResponse(null, { status: 204 });
			})
		])
	);
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	accountStore.setConnected('anilist', as('user-1'));
	target = document.createElement('div');
	document.body.appendChild(target);
	scaled.length = 0;
	writes = 0;
	for (const item of [...toastStore.items]) toastStore.dismiss(item.id);
});

afterEach(() => {
	if (app) unmount(app);
	app = null;
	target.remove();
	accountStore.setDisconnected('anilist');
});

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

const railTitles = () =>
	Array.from(
		target.querySelectorAll(`[aria-label="${m.account_watch_later_title()}"] .card-title`)
	).map((el) => el.textContent?.trim() ?? '');

const save = () =>
	syncSetEntry(String(NEW.malId), { status: 'planning', statusChanged: true, progress: 0 });

describe('Watch Later on Home, while a held write waits', () => {
	it('takes in a show whose held add lands, growing it into the row as Continue Watching moves', async () => {
		planned = [ALREADY];
		backend();
		const pending = save();
		// The user goes Home while the add waits out AniList's pause.
		app = mount(HomePage, { target });
		await settle();
		expect(railTitles()).toEqual([ALREADY.title]);

		await pending;
		await settle();
		expect(writes).toBe(2);
		expect(railTitles()).toEqual([NEW.title, ALREADY.title]);
		// The new card grew in with the strip's motion; nothing else did.
		expect(scaled).toEqual([{ id: String(NEW.malId), duration: 240 }]);
	});

	it('lets go of a show whose held removal lands, shrinking it out of the row', async () => {
		planned = [ALREADY, NEW];
		backend();
		const pending = syncRemoveEntry(String(NEW.malId));
		app = mount(HomePage, { target });
		await settle();
		expect(railTitles()).toEqual([NEW.title, ALREADY.title]);

		await pending;
		await settle();
		expect(writes).toBe(2);
		expect(railTitles()).toEqual([ALREADY.title]);
		expect(scaled).toEqual([{ id: String(NEW.malId), duration: 240 }]);
	});

	it('grows the first card into an empty row, the same way', async () => {
		planned = [];
		backend();
		const pending = save();
		app = mount(HomePage, { target });
		await settle();
		expect(railTitles()).toEqual([]);
		expect(target.textContent).toContain(m.account_watch_later_empty());

		await pending;
		await settle();
		expect(railTitles()).toEqual([NEW.title]);
		expect(scaled).toEqual([{ id: String(NEW.malId), duration: 240 }]);
	});

	it('shrinks the last card out, leaving the empty row', async () => {
		planned = [NEW];
		backend();
		const pending = syncRemoveEntry(String(NEW.malId));
		app = mount(HomePage, { target });
		await settle();
		expect(railTitles()).toEqual([NEW.title]);

		await pending;
		await settle();
		expect(railTitles()).toEqual([]);
		expect(target.textContent).toContain(m.account_watch_later_empty());
		expect(scaled).toEqual([{ id: String(NEW.malId), duration: 240 }]);
	});

	it('changes nothing when the held add is dropped for another account', async () => {
		planned = [ALREADY];
		backend();
		const pending = save();
		app = mount(HomePage, { target });
		await settle();
		accountStore.setConnected('anilist', as('user-2'));
		await pending;
		await settle();
		expect(writes).toBe(1);
		expect(railTitles()).toEqual([ALREADY.title]);
		expect(scaled).toEqual([]);
	});

	it('reads the new list, without motion, when Home is opened after the write landed', async () => {
		planned = [ALREADY];
		backend();
		await save();
		app = mount(HomePage, { target });
		await settle();
		expect(railTitles()).toEqual([NEW.title, ALREADY.title]);
		expect(scaled).toEqual([]);
	});
});
