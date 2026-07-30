// Acceptance: the Watch Later rail renders newest-planned first.
//
// The ordering itself is unit-covered on the merge helper, but the
// helper is four steps away from the rail. Between them sit the route's
// credential assembly, the loader's bridge call, the Kitsu bridge that
// turns tracker ids into cards, and the availability filter that drops
// some of them — any of which could reorder or re-sort, and none of
// which the helper's tests can see.
//
// So this asserts the order twice, at both ends of that path: the ids
// the loader asks the bridge for, and the titles the user ends up
// looking at. The tracker deliberately answers in an order that is
// neither, so a route that simply forwarded what it was given would
// fail both.

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

import HomePage from '../../src/routes/+page.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { accountStore } from '../../src/lib/account/store.svelte';
import type { ListEntry } from '../../src/lib/account/types';

/** Planned titles, and when the user planned them. Declared oldest to
 *  newest so the expected rail order is this list reversed — writing it
 *  the other way round would make the fixture agree with the assertion
 *  by construction. */
const PLANNED = [
	{ malId: 11, title: 'Planned Long Ago', updated: 1_700_000_000 },
	{ malId: 12, title: 'Planned Last Month', updated: 1_700_000_100 },
	{ malId: 13, title: 'Planned Yesterday', updated: 1_700_000_200 }
];
const NEWEST_FIRST = [...PLANNED].reverse().map((p) => p.title);

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;

function listEntry(malId: number, title: string, updated: number): ListEntry {
	return {
		provider: 'anilist',
		media_id: malId,
		mal_id: malId,
		status: 'planning',
		progress_episodes: 0,
		score_0_to_100: null,
		updated_at_epoch_s: updated,
		title
	};
}

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	// The rail loads only for a connected tracker. `hydrate()` reads the
	// Electron keychain and runs from the layout, which a page-level
	// mount never reaches — so seed the store the way hydrate would.
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

async function settle(steps = 40) {
	for (let i = 0; i < steps; i++) await new Promise((r) => setTimeout(r, 5));
}

/** Card titles in the order the rail renders them. */
const railTitles = () =>
	Array.from(target.querySelectorAll('.poster-card .card-title')).map(
		(el) => el.textContent?.trim() ?? ''
	);

describe('Watch Later rail order', () => {
	it('renders the most recently planned title first, all the way to the card', async () => {
		/** The mal_ids the loader asked the Kitsu bridge for, in order. */
		let bridged: number[] = [];

		server.use(
			...homeHandlers({}, [
				http.get(`${API_BASE}/api/account/list/anilist/cached`, () =>
					// Deliberately oldest-first, which is neither the order
					// the rail should render nor a reversal of it — the
					// tracker's own order must not be able to pass for the
					// rail's.
					HttpResponse.json(PLANNED.map((p) => listEntry(p.malId, p.title, p.updated)))
				),
				http.post(`${API_BASE}/api/kitsu/by-mal-ids`, async ({ request }) => {
					const { mal_ids } = (await request.json()) as { mal_ids: number[] };
					bridged = mal_ids;
					// Slot-for-slot, the way the backend bridge answers —
					// it fills one slot per requested id precisely so the
					// caller's order survives the round trip.
					return HttpResponse.json(
						mal_ids
							.map((id) => PLANNED.find((p) => p.malId === id))
							.filter((p): p is (typeof PLANNED)[number] => p !== undefined)
							.map((p) => kitsuRef(String(p.malId), p.title, 12) as KitsuRefFixture)
					);
				}),
				// Nothing is pruned, so what renders is purely a question
				// of order.
				http.post(`${API_BASE}/api/availability/batch`, () => HttpResponse.json({ cached: {} }))
			])
		);

		app = mount(HomePage, { target });
		await settle();

		// End one: what the route asked the bridge for. This is the order
		// the 500-id cap would truncate, so it matters even for a library
		// too large to render.
		expect(bridged, 'the bridge should be asked newest-first').toEqual(
			[...PLANNED].reverse().map((p) => p.malId)
		);

		// End two: what the user sees. Everything between the two ends —
		// the bridge round trip, the availability filter, the Strip — has
		// to leave the order alone for both to hold.
		expect(railTitles()).toEqual(NEWEST_FIRST);
	});
});
