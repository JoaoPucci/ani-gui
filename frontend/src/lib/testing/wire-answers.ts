// Wire answers as the tests fake them: the renderer's types for what
// the backend sends, built whole so a fake names only the fields its
// test is about. What a builder fills is a value the backend sends:
// empty where the field can be (`null`, `[]`, `false`), the backend's
// own defaults for the settings, and plain placeholders for the few
// fields that cannot be empty (an id, a title, a version). Test code
// only: ESLint refuses an import of `$lib/testing` from app code.
//
// The MSW side (an error answer, an answer wrong on purpose, and the
// answers only the acceptance tier builds) is `frontend/tests/answers.ts`,
// which re-exports these.

import { AccountApiError } from '$lib/account/api';
import type {
	AniErrorPayload,
	AppInfo,
	AvailabilityResponse,
	Config,
	CreateSessionResponse,
	KitsuAnimeRef,
	KitsuEpisode
} from '$lib/api';

/** A value no backend sends, deliberately, typed as the one it stands
 *  in for: a unit test of what the code does with a stale or broken
 *  shape. */
export function malformedAs<T>(value: unknown): T {
	return value as T;
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

/** A Kitsu show as the backend sends one, with `fields` over a show
 *  that carries no metadata beyond its id and title. */
export function kitsuRefAnswer(fields: Partial<KitsuAnimeRef> = {}): KitsuAnimeRef {
	return {
		id: '1',
		canonical_title: 'Title', // i18n-ignore: test data, not copy
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

/** A stream event's data as the backend sends it, typed as the event
 *  carries it: `wire<PlayProgress>({ kind: 'matched', title })`. The
 *  type is required: without one the payload types as nothing. */
export function wire<T = never>(payload: NoInfer<T>): string {
	return JSON.stringify(payload);
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

/** An error as the backend sends it and `$lib/api` rejects with it: the
 *  envelope, typed, for a fake rejection or a thrown answer. */
export function errorEnvelope(envelope: AniErrorPayload): AniErrorPayload {
	return envelope;
}

/** An account route's refusal as `$lib/account/api` rejects with it: the
 *  status, and the backend's error envelope as the body. */
export function accountRefusal(status: number, envelope: AniErrorPayload): AccountApiError {
	return new AccountApiError(status, JSON.stringify(envelope));
}
