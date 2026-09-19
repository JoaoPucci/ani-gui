// Acceptance: a session's sidecar subtitle tracks reach the player.
//
// hianime delivers subtitles as .vtt sidecars outside the playlist, so
// a sub stream is raw video unless the page attaches them. The page
// keeps only the session id across navigation, so it asks the proxy
// for the session's tracks and appends one <track> per listing to the
// singleton video — where the browser renders them natively and the
// captions picker already lists them. Switching sessions replaces the
// tracks; a session with none attaches none.
//
// mp4 sessions go straight to the element's `src`, which is what
// happy-dom can carry; the track handling does not care which kind.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { http, HttpResponse } from 'msw';
import { mount, unmount } from 'svelte';
import { server, API_BASE } from './setup';
import { page, setParams, setUrl } from './page-state.svelte';
import { kitsuRef, appConfig } from './home-handlers';
import { __resetApiBaseForTests } from '../../src/lib/api';
import { getGlobalVideo } from '../../src/lib/play/global-video';
import PlayPage from '../../src/routes/play/[id]/+page.svelte';

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

function kitsuEpisodes(count: number) {
	return Array.from({ length: count }, (_, i) => ({
		id: `ep-${i + 1}`,
		number: i + 1,
		canonical_title: `Episode ${i + 1}`,
		thumbnail: null,
		synopsis: null
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

function tracksOf(video: HTMLVideoElement) {
	return Array.from(video.querySelectorAll('track')).map((t) => ({
		srclang: t.getAttribute('srclang'),
		label: t.getAttribute('label'),
		src: t.getAttribute('src'),
		isDefault: t.hasAttribute('default')
	}));
}

describe('sidecar subtitle tracks', () => {
	it("attaches the session's tracks and replaces them on the next session", async () => {
		server.use(
			http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig())),
			http.get(`${API_BASE}/api/kitsu/anime/${KITSU_ID}`, () =>
				HttpResponse.json(kitsuRef(KITSU_ID, 'The Show', 12))
			),
			http.get(`${API_BASE}/api/kitsu/airing/${KITSU_ID}`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json(kitsuEpisodes(12))),
			http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
			http.post(`${API_BASE}/api/availability`, () =>
				HttpResponse.json({ available: true, episode_count: 12, extra_episodes: [] })
			),
			http.post(`${API_BASE}/api/play/mark-watched`, () => new HttpResponse(null, { status: 204 })),
			http.get(`${API_BASE}/api/aniskip/:id/:episode`, () => HttpResponse.json(null)),
			http.get(`${API_BASE}/s/session-1/subtitles`, () =>
				HttpResponse.json([
					{
						lang: 'en',
						label: 'English',
						default: true,
						url: `${API_BASE}/s/session-1/sub/0.vtt`
					},
					{
						lang: 'es',
						label: 'Español',
						default: false,
						url: `${API_BASE}/s/session-1/sub/1.vtt`
					}
				])
			),
			http.get(`${API_BASE}/s/session-2/subtitles`, () => HttpResponse.json([]))
		);
		setUrl(`/play/${KITSU_ID}`, { session: 'session-1', episode: '1', kind: 'mp4' });
		app = mount(PlayPage, { target });

		const video = getGlobalVideo();
		await until(() => video.querySelectorAll('track').length === 2, 'both tracks attached');
		expect(tracksOf(video)).toEqual([
			{
				srclang: 'en',
				label: 'English',
				src: `${API_BASE}/s/session-1/sub/0.vtt`,
				isDefault: true
			},
			{
				srclang: 'es',
				label: 'Español',
				src: `${API_BASE}/s/session-1/sub/1.vtt`,
				isDefault: false
			}
		]);

		// The next episode resolved to a session with no tracks: the
		// old ones must not linger on the singleton.
		setUrl(`/play/${KITSU_ID}`, { session: 'session-2', episode: '2', kind: 'mp4' });
		await until(() => video.src.includes('session-2'), 'the second session attached');
		await until(
			() => video.querySelectorAll('track').length === 0,
			"the first session's tracks removed"
		);
	});
});
