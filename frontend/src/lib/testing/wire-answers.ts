// Wire answers as the unit tests fake them: the renderer's types for
// what the backend sends, built whole so a fake names only the fields
// its test is about. A field they fill is the value the backend sends
// when it has nothing to say (`null`, empty, `false`); the settings
// builder is the backend's own defaults. Test code only: nothing in
// the app imports this module.
//
// The MSW side (an error answer, an answer wrong on purpose, and the
// answers only the acceptance tier builds) is `frontend/tests/answers.ts`,
// which re-exports these.

import type { AvailabilityResponse, Config, KitsuAnimeRef, KitsuEpisode } from '$lib/api';

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
