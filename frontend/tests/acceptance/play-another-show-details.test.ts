// Acceptance: the player reached from another show's player shows the
// new show's details, not the old one's.
//
// The player reads the show it plays once, as its page opens: the
// title, the episode count behind Next, the airing schedule that greys
// the episodes still to come. Every way from one show's player to
// another's lands on the same route with another id (a switch to the
// show picked for a guessed card, a step back or forward between two
// shows' players), which SvelteKit would serve with the page it already
// has. The route's layout opens a new page for another show
// (./play-route-remount.test.ts pins the layout alone); this walks the
// layout and the real page through that navigation and asks for the
// second show's details where the viewer sees them.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';

import { API_BASE, server } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
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

import PlayRoute from './PlayRoute.svelte';
import { __resetApiBaseForTests } from '../../src/lib/api';

import type {
	AiringStatus,
	AvailabilityResponse,
	Config,
	KitsuAnimeRef,
	KitsuEpisode,
	SkipInterval
} from '../../src/lib/api';
import { availabilityAnswer } from '../answers';

/** The show playing first: finished, three episodes, the last playing. */
const FIRST = '4013';
const FIRST_TITLE = 'The Short';
const FIRST_EPISODES = 3;
/** The show reached from it: airing, three of twelve episodes out. */
const SECOND = '7158';
const SECOND_TITLE = 'The Series';
const SECOND_EPISODES = 12;
const SECOND_AIRED = 3;

function episodes(count: number): KitsuEpisode[] {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		thumbnail: null,
		synopsis: null,
		season_number: null,
		relative_number: i + 1,
		length: null,
		airdate: null
	}));
}

function show(id: string): KitsuAnimeRef {
	return id === SECOND
		? { ...kitsuRef(SECOND, SECOND_TITLE, SECOND_EPISODES), status: 'current' }
		: kitsuRef(FIRST, FIRST_TITLE, FIRST_EPISODES);
}

let target: HTMLElement;
let app: ReturnType<typeof mount> | null = null;
let airingAsked: string[];

beforeEach(() => {
	__resetApiBaseForTests(API_BASE);
	airingAsked = [];
	target = document.createElement('div');
	document.body.appendChild(target);
	server.use(
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig())),
		http.get(`${API_BASE}/api/kitsu/anime/:id`, ({ params }) =>
			HttpResponse.json<KitsuAnimeRef>(show(String(params.id)))
		),
		http.get(`${API_BASE}/api/kitsu/airing/:id`, ({ params }) => {
			airingAsked.push(String(params.id));
			return HttpResponse.json<AiringStatus>({
				aired: SECOND_AIRED,
				next_episode: SECOND_AIRED + 1,
				next_airing_at: null,
				upcoming: []
			});
		}),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, ({ params }) =>
			HttpResponse.json<KitsuEpisode[]>(
				episodes(String(params.id) === SECOND ? SECOND_EPISODES : FIRST_EPISODES)
			)
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.post(`${API_BASE}/api/availability`, async ({ request }) => {
			const body = (await request.json()) as { kitsu_id?: string };
			return HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({
					available: true,
					episode_count: body.kitsu_id === SECOND ? SECOND_EPISODES : FIRST_EPISODES,
					extra_episodes: [],
					episode_count_approximate: false
				})
			);
		}),
		http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json<SkipInterval[]>([]))
	);
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

const title = () => target.querySelector('a.np-link')?.textContent ?? '';
const next = () =>
	target.querySelector<HTMLButtonElement>(
		`button[aria-label="${m.play_episode_nav_next_aria_label()}"]`
	);
const tile = (n: number) =>
	target.querySelector<HTMLButtonElement>(`li[data-ep-num="${n}"] button`);

describe("the player reached from another show's player", () => {
	it("shows the new show's title, episode count and airing schedule", async () => {
		setParams({ id: FIRST });
		setUrl(`/play/${FIRST}`, { episode: String(FIRST_EPISODES) });
		app = mount(PlayRoute, { target });
		await until(() => title().includes(FIRST_TITLE), "the first show's title");
		await until(() => tile(FIRST_EPISODES) !== null && next() !== null, "the first show's strip");
		// Its last episode: nothing follows it. A finished show asks for
		// no schedule.
		expect(next()!.disabled).toBe(true);
		expect(tile(FIRST_EPISODES + 1)).toBeNull();
		expect(airingAsked).toEqual([]);

		// The same route, another show, as SvelteKit's navigation moves
		// the route param.
		setParams({ id: SECOND });
		setUrl(`/play/${SECOND}`, { episode: '1' });

		await until(() => title().includes(SECOND_TITLE), "the second show's title");
		expect(title()).not.toContain(FIRST_TITLE);
		// The strip counts the second show's twelve episodes, the ones
		// still to come greyed by its schedule.
		await until(
			() => tile(SECOND_AIRED + 1)?.classList.contains('ep-card-unaired') === true,
			"the second show's schedule"
		);
		expect(target.textContent).toContain(`of ${SECOND_EPISODES}`);
		expect(airingAsked).toEqual([SECOND]);
		expect(tile(SECOND_AIRED)!.classList.contains('ep-card-unaired')).toBe(false);
		// Episode two follows: Next counts the second show's episodes.
		await until(() => next()!.disabled === false, 'Next to open for episode two');
		expect(next()!.textContent).toContain('2');
	});
});
