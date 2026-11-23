// Acceptance: a webview without HLS support shows the player's
// error overlay and leaves it there.
//
// The media effect attaches the source and, when neither hls.js nor
// native HLS is available, sets the player error instead. The same
// effect clears that error at its start, so an effect that also
// READS it afterwards subscribes to its own write and reruns until
// Svelte gives up with its update-depth error — instead of the
// overlay staying put. happy-dom has no MediaSource and no native
// HLS, which is exactly that webview.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';

import type {
	AiringStatus,
	AvailabilityResponse,
	Config,
	KitsuAnimeRef,
	KitsuEpisode,
	SessionSubtitle,
	SkipInterval
} from '../../src/lib/api';
import { availabilityAnswer } from '../answers';
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

const KITSU_ID = '42';

function kitsuEpisodes(count: number): KitsuEpisode[] {
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
	const start = Date.now();
	while (!predicate()) {
		if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for ${what}`);
		await new Promise((r) => setTimeout(r, 10));
	}
}

describe('a webview without HLS support', () => {
	it('shows the unsupported-HLS overlay and keeps it', async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json<KitsuAnimeRef>(kitsuRef(KITSU_ID, 'The Show', 12))
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () =>
				HttpResponse.json<AiringStatus>(null)
			),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () =>
				HttpResponse.json<KitsuEpisode[]>(kitsuEpisodes(12))
			),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json<AvailabilityResponse>(
					availabilityAnswer({
						available: true,
						episode_count: 12,
						extra_episodes: []
					})
				)
			),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json<SkipInterval[]>([])),
			http.get(`${API_BASE}/s/session-1/subtitles`, () => HttpResponse.json<SessionSubtitle[]>([]))
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'hls' });
		app = mount(PlayPage, { target });

		await until(() => target.querySelector('.player-error-detail') !== null, 'the error overlay');
		// Long enough for a rerunning effect to exhaust Svelte's update
		// depth. Svelte throws that from its flush, outside the test
		// body, and vitest fails the run on the unhandled error — the
		// witness this test relies on; a stable overlay just sits here.
		await new Promise((r) => setTimeout(r, 200));
		expect(target.querySelector('.player-error-detail')?.textContent).toContain(
			'HLS playback is not supported'
		);
	});
});
