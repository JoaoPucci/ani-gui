// The backend's answers as the tests fake them.
//
// A fake answer is typed against the renderer's own type for that
// route, so a fake that drifts from what the backend sends fails
// type-checking instead of passing a test on a shape no backend
// produces. A success answer names its type at the call:
// `HttpResponse.json<KitsuAnimeRef[]>([...])` (MSW reads the type
// argument, and checks the body against it, with `NoInfer`). The two
// helpers here cover the rest:
//
// - `errorAnswer` is an error answer: a status and the error envelope the
//   backend sends with it, or `null` for an answer with no envelope.
// - `availabilityAnswer`, `appInfoAnswer`, `sessionAnswer`,
//   `kitsuRefAnswer`, `episodeAnswer` and `configAnswer` build an answer
//   with every field the backend always sends, so a fake names only the
//   ones its test is about. A field they fill is the value the backend
//   sends when it has nothing to say (`null`, empty, `false`).
// - `malformedAnswer` is an answer no backend sends, on purpose: the test is
//   about what the renderer does with one. Using it says so.
//
// The ESLint config refuses an `HttpResponse.json` call without a type
// argument in test code, so a new fake cannot be left untyped.

import { HttpResponse, type JsonBodyType } from 'msw';

import type {
	AniErrorPayload,
	AppInfo,
	AvailabilityResponse,
	Config,
	CreateSessionResponse,
	KitsuAnimeRef,
	KitsuEpisode
} from '../src/lib/api';

/** An error answer: `status`, with `envelope` as its body (`null` for
 *  one with no envelope). It fits any handler, whatever the route's
 *  success type. */
export function errorAnswer(
	status: number,
	envelope: AniErrorPayload | null = null
): HttpResponse<never> {
	return HttpResponse.json<AniErrorPayload | null>(envelope, { status }) as HttpResponse<never>;
}

/** A value no backend sends, deliberately, typed as the one it stands
 *  in for: a unit test of what the code does with a stale or broken
 *  shape. */
export function malformedAs<T>(value: unknown): T {
	return value as T;
}

/** An answer no backend sends, deliberately: its body is not the
 *  route's type. */
export function malformedAnswer(body: unknown, init?: ResponseInit): HttpResponse<never> {
	return HttpResponse.json<JsonBodyType>(body as JsonBodyType, init) as HttpResponse<never>;
}

/** A probe answer as the backend sends one: `fields`, and every field
 *  it always sends that `fields` leaves out at its settled default. A
 *  positive answer names the provider that carries the show. */
export function availabilityAnswer(
	fields: Partial<AvailabilityResponse> = {}
): AvailabilityResponse {
	const available = fields.available ?? false;
	return {
		available,
		episode_count: null,
		extra_episodes: [],
		episode_count_approximate: false,
		provider: available ? 'hianime' : null,
		...fields
	};
}

/** `/api/app-info` as the backend sends it on a launch whose sweeps
 *  found nothing, with `fields` over it. */
export function appInfoAnswer(fields: Partial<AppInfo> = {}): AppInfo {
	return {
		version: '0.0.0-test',
		history_path: '/tmp/history',
		proxy_base_url: 'http://127.0.0.1:0',
		removed_legacy_paths: [],
		retired_provider_sweep: { superseded: [], renumbered: [], numbering_rows: 0, cache_rows: 0 },
		cache_reclaim: { undeclared_rows: 0, expired_rows: 0 },
		...fields
	};
}

/** A play's session as the backend sends it: a fresh resolve with no
 *  sidecar subtitles, with `fields` over it. */
export function sessionAnswer(
	fields: Pick<CreateSessionResponse, 'session_id' | 'media_url' | 'media_kind'> &
		Partial<CreateSessionResponse>
): CreateSessionResponse {
	return { cache_hit: false, subtitles: [], ...fields };
}

/** A Kitsu show as the backend sends one, with `fields` over a show
 *  that carries no metadata beyond its id and title. */
export function kitsuRefAnswer(fields: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id: '1',
		canonical_title: 'Title',
		titles: {},
		abbreviated_titles: [],
		slug: null,
		synopsis: null,
		start_date: null,
		end_date: null,
		episode_count: null,
		average_rating: null,
		subtype: null,
		status: null,
		age_rating: null,
		popularity_rank: null,
		poster_image: null,
		cover_image: null,
		...fields
	};
}

/** One Kitsu episode as the backend sends it, with `fields` over an
 *  episode that carries nothing beyond its id. */
export function episodeAnswer(fields: Partial<KitsuEpisode> = {}): KitsuEpisode {
	return {
		id: 'ep-1',
		canonical_title: null,
		season_number: null,
		number: null,
		relative_number: null,
		length: null,
		synopsis: null,
		airdate: null,
		thumbnail: null,
		...fields
	};
}

/** `/api/settings` as the backend sends a fresh Linux install's
 *  (`Config::default()`), with `fields` over it. */
export function configAnswer(fields: Partial<Config> = {}): Config {
	return {
		locale: 'en',
		mode: 'sub',
		download_quality: 'best',
		external_player: 'mpv',
		external_player_kind: 'mpv',
		external_player_custom_args: '',
		syncplay_binary: 'syncplay',
		image_cache_cap_mb: 500,
		auto_play_next: false,
		cache_resolutions: false,
		download_bottom_bar_enabled: true,
		auto_skip_op: false,
		auto_skip_ed: false,
		use_custom_player_controls: true,
		update_include_prereleases: true,
		primary_account: '',
		...fields
	};
}
