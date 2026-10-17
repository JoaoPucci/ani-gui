// Acceptance: an availability answer that settled nothing leaves the
// page's play surfaces as they were — unless what it says is that the
// catalogue refused the show by name, which is the show not being in
// the catalogue.
//
// An entry Kitsu counts but does not date is listed on the provider
// under none of its names, and the search answers with other shows.
// The probe refuses them on how they are named and comes back without
// a verdict — delivered as a normal answer that says `unsettled`, where
// it used to be an HTTP 404 the page caught and read as "could not
// tell". The answer carries `available: false` and means nothing by it,
// so the page must read it as the unknown it is: no "not in the
// catalogue" notice, the Play action rendered for the lazy path to
// surface the real verdict on a click.

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

const KITSU_ID = '45637';
const TITLE = 'Mo Du Zhe';

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

/** The notice the page shows for a show the catalogue does not carry. */
const unavailableNotice = () => target.querySelector('p.unavailable[role="status"]');
/** The Play action, rendered only once the page has a show to play. */
const playAction = () => target.querySelector('.actions button.btn');

function routes(unsettled: unknown, onProbe: () => void) {
	return [
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
		// Counted, undated: the entry the count-alone rule is for.
		http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
			HttpResponse.json({
				...kitsuRef(KITSU_ID, TITLE, 12),
				status: 'current',
				start_date: null
			})
		),
		http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
			HttpResponse.json({ aired: 5, next_episode: 6, next_airing_at: null, upcoming: [] })
		),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () => {
			onProbe();
			return HttpResponse.json({
				available: false,
				episode_count: null,
				extra_episodes: [],
				unsettled
			});
		})
	];
}

describe('detail route — an availability answer that settled nothing', () => {
	for (const unsettled of [true, 'unanswered', 'past_affinity', 'superseded']) {
		it(`keeps the play action and shows no not-in-catalogue notice (${String(unsettled)})`, async () => {
			let probes = 0;
			server.use(...routes(unsettled, () => (probes += 1)));

			app = mount(DetailPage, { target });
			await until(() => probes > 0, 'the page-load availability lookup');
			await until(() => playAction() !== null, 'the play action to render');

			// The answer settled nothing, so nothing it says may read as
			// the show being gone.
			expect(unavailableNotice()).toBeNull();
		});
	}

	it('shows the not-in-catalogue notice, and no play action, for a refusal by name', async () => {
		// HenSemi (TV)'s shape: the provider lists nothing named for the
		// entry, so a Play button could only fail.
		let probes = 0;
		server.use(...routes('refused', () => (probes += 1)));

		app = mount(DetailPage, { target });
		await until(() => probes > 0, 'the page-load availability lookup');
		await until(() => unavailableNotice() !== null, 'the not-in-catalogue notice');

		expect(playAction()).toBeNull();
	});
});

describe('detail route — the answer as a backend before reasons sent it', () => {
	it('keeps the play action for `unsettled: true`', async () => {
		let probes = 0;

		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			// Counted, undated: the entry the rule is for.
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json({
					...kitsuRef(KITSU_ID, TITLE, 12),
					status: 'current',
					start_date: null
				})
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json({ aired: 5, next_episode: 6, next_airing_at: null, upcoming: [] })
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () => {
				probes += 1;
				return HttpResponse.json({
					available: false,
					episode_count: null,
					extra_episodes: [],
					unsettled: true
				});
			})
		);

		app = mount(DetailPage, { target });
		await until(() => probes > 0, 'the page-load availability lookup');
		await until(() => playAction() !== null, 'the play action to render');

		// The answer settled nothing, so nothing it says may read as the
		// show being gone.
		expect(unavailableNotice()).toBeNull();
	});
});
