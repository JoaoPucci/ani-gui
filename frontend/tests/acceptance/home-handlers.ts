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

import type {
	AvailabilityResponse,
	Config,
	HistoryEntry,
	HistoryNumber,
	KitsuAnimeRef,
	KitsuEpisode
} from '../../src/lib/api';

import { availabilityAnswer } from '../answers';
/** A Kitsu ref as the backend sends one: every field, the ones the
 *  home surfaces do not read left empty. */
export function kitsuRef(id: string, title: string, episodeCount: number): KitsuAnimeRef {
	return {
		id,
		canonical_title: title,
		titles: {},
		abbreviated_titles: [],
		slug: title.toLowerCase().replace(/\s+/g, '-'),
		synopsis: null,
		start_date: '2019-01-01',
		end_date: null,
		episode_count: episodeCount,
		average_rating: null,
		subtype: null,
		status: 'finished',
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null
	};
}

export function appConfig(mode: 'sub' | 'dub' = 'sub'): Config {
	return {
		locale: 'en',
		mode,
		download_quality: 'best',
		external_player: '',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: '',
		image_cache_cap_mb: 100,
		auto_play_next: false,
		cache_resolutions: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: true,
		update_include_prereleases: false,
		primary_account: '',
		watch_order: 'linked'
	};
}

export interface HomeHandlerOptions {
	/** History rows the route loads on mount. Defaults to none. */
	history?: HistoryEntry[];
	mode?: 'sub' | 'dub';
}

/**
 * Everything a mounted home route reaches for, answered the way a cold
 * cache would: no stored the provider→Kitsu mapping, no remembered title
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
		http.get(`${API_BASE}/api/settings`, () => HttpResponse.json<Config>(appConfig(mode))),
		http.get(`${API_BASE}/api/history`, () => HttpResponse.json<HistoryEntry[]>(history)),
		// A row read again — by a card whose probe answered, or by a
		// click — reads as the fixture has it.
		http.get(`${API_BASE}/api/history/:id/number`, ({ params }) => {
			const row = history.find((h) => h.id === params.id);
			return HttpResponse.json<HistoryNumber | null>(row ? { ep_no: row.ep_no } : null);
		}),
		http.get(`${API_BASE}/api/watched-at`, () => HttpResponse.json<Record<string, number>>({})),
		http.get(`${API_BASE}/api/kitsu/trending-anilist`, () =>
			HttpResponse.json<KitsuAnimeRef[]>([])
		),
		http.get(`${API_BASE}/api/kitsu/top-rated`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		http.get(`${API_BASE}/api/allmanga-kitsu-map/:showId`, () =>
			HttpResponse.json<string | null>(null)
		),
		http.get(`${API_BASE}/api/title-match`, () => HttpResponse.json<string | null>(null)),
		http.put(`${API_BASE}/api/title-match`, () => new HttpResponse(null, { status: 204 })),
		http.get(`${API_BASE}/api/kitsu/episodes/:id`, () => HttpResponse.json<KitsuEpisode[]>([])),
		// Step 4 of the history resolver: reached only when the reverse
		// cache, the remembered title match and the Kitsu search have
		// all whiffed. Unstubbed it throws, the resolver swallows that
		// and returns null, and the row renders its /search fallback —
		// a state a scenario about the probe window must not reach by
		// accident.
		http.get(`${API_BASE}/api/kitsu/resolve-allmanga/:showId`, () =>
			HttpResponse.json<KitsuAnimeRef | null>(null)
		),
		http.post(`${API_BASE}/api/kitsu/search`, () => HttpResponse.json<KitsuAnimeRef[]>([])),
		// The hits a guessed row rules out, asked of the backend between
		// the search and the pick: none, unless a scenario says.
		http.post(`${API_BASE}/api/history/guess/ruled-out`, () => HttpResponse.json<string[]>([])),
		http.post(`${API_BASE}/api/availability`, () =>
			HttpResponse.json<AvailabilityResponse>(
				availabilityAnswer({ available: true, episode_count: 26 })
			)
		)
	];
}
