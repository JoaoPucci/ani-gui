// The request surface of a mounted home route, in one place.
//
// A scenario that misses one of these does not fail loudly: the route
// and the history resolver catch their own request failures, so an
// unstubbed endpoint silently moves the run onto a fallback path while
// `onUnhandledRequest: 'error'` reports into the void. Keeping the list
// here means a scenario opts out of a default deliberately rather than
// by forgetting.

import { http, HttpResponse, type HttpHandler } from 'msw';

import { API_BASE } from './setup';

export interface KitsuRefFixture {
	id: string;
	canonical_title: string;
	titles: Record<string, string>;
	abbreviated_titles: string[];
	slug: string;
	synopsis: string | null;
	poster_image: string | null;
	cover_image: string | null;
	episode_count: number;
	status: string;
	start_date: string;
	average_rating: string | null;
}

/** A Kitsu ref carrying only the fields the home surfaces read. */
export function kitsuRef(id: string, title: string, episodeCount: number): KitsuRefFixture {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		poster_image: null,
		cover_image: null,
		episode_count: episodeCount,
		status: 'finished',
		start_date: '2019-01-01',
		average_rating: null
	};
}

export function appConfig(mode: 'sub' | 'dub' = 'sub') {
	return {
		locale: 'en',
		mode,
		quality: 'best',
		external_player: '',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: '',
		image_cache_cap_mb: 100,
		auto_play_next: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: true,
		disable_auto_pip_on_leave: false,
		auto_update_anicli: false,
		update_include_prereleases: false,
		primary_account: ''
	};
}

export interface HomeHandlerOptions {
	/** History rows the route loads on mount. Defaults to none. */
	history?: { ep_no: string; id: string; title: string }[];
	mode?: 'sub' | 'dub';
}

/**
 * Everything a mounted home route reaches for, answered the way a cold
 * cache would: no stored allanime→Kitsu mapping, no remembered title
 * match, no reverse-resolve, empty rails.
 *
 * Scenario-specific handlers go in `overrides`, and this puts them
 * ahead of the defaults. Within a single `server.use(a, b)` call MSW
 * answers with the FIRST matching handler, so a scenario that appended
 * its override after a default silently kept the default — measured,
 * not assumed. Taking overrides as a parameter removes the ordering
 * from call sites entirely.
 */
export function homeHandlers(
	opts: HomeHandlerOptions = {},
	overrides: HttpHandler[] = []
): HttpHandler[] {
	const { history = [], mode = 'sub' } = opts;
	return [
		...overrides,
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json(appConfig(mode))),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json(history)),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json({})),
		http.get(`${API_BASE}/api/kitsu/trending-anilist`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json([])),
		http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () => HttpResponse.json(null)),
		http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json(null)),
		http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json([])),
		// Step 4 of the history resolver: reached only when the reverse
		// cache, the remembered title match and the Kitsu search have
		// all whiffed. Unstubbed it throws, the resolver swallows that
		// and returns null, and the row renders its /search fallback —
		// a state a scenario about the probe window must not reach by
		// accident.
		http.get(`${API_BASE}/api/kitsu/resolve-allmanga/:showId`, () => HttpResponse.json(null)),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json({ available: true, episode_count: 26, approximate: false })
		)
	];
}
