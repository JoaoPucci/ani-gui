/**
 * Hermetic Kitsu / history fixtures used by Playwright tests.
 *
 * The real Rust backend at runtime hits Kitsu over the network and
 * returns shapes our frontend understands. For tests we don't want
 * that — we stub the renderer's `fetch()` calls via `page.route()`
 * so the assertions run against a fixed-shape response and don't
 * depend on Kitsu's availability or schema changes.
 *
 * Each fixture matches the shape of `KitsuAnimeRef` / `HistoryEntry`
 * in `gui/frontend/src/lib/api.ts` — keep them in sync if those
 * types ever change.
 */

export const trending = [
	{
		id: '1',
		canonical_title: 'Cowboy Bebop',
		english_title: 'Cowboy Bebop',
		synopsis: 'Spike Spiegel and his crew chase bounties across the solar system.',
		episode_count: 26,
		average_rating: '85.34',
		start_date: '1998-04-03',
		status: 'finished',
		poster_image: {
			tiny: 'https://media.kitsu.app/anime/poster_images/1/tiny.jpg',
			small: 'https://media.kitsu.app/anime/poster_images/1/small.jpg',
			medium: 'https://media.kitsu.app/anime/poster_images/1/medium.jpg',
			large: 'https://media.kitsu.app/anime/poster_images/1/large.jpg',
			original: 'https://media.kitsu.app/anime/poster_images/1/original.jpg'
		},
		cover_image: {
			tiny: null,
			small: null,
			large: 'https://media.kitsu.app/anime/cover_images/1/large.jpg',
			original: 'https://media.kitsu.app/anime/cover_images/1/original.jpg'
		}
	}
];

export const topRated = [
	{
		id: '11',
		canonical_title: 'Fullmetal Alchemist: Brotherhood',
		english_title: 'Fullmetal Alchemist: Brotherhood',
		synopsis: 'Two brothers search for the Philosopher’s Stone.',
		episode_count: 64,
		average_rating: '90.86',
		start_date: '2009-04-05',
		status: 'finished',
		poster_image: {
			tiny: 'https://media.kitsu.app/anime/poster_images/11/tiny.jpg',
			small: 'https://media.kitsu.app/anime/poster_images/11/small.jpg',
			medium: 'https://media.kitsu.app/anime/poster_images/11/medium.jpg',
			large: 'https://media.kitsu.app/anime/poster_images/11/large.jpg',
			original: 'https://media.kitsu.app/anime/poster_images/11/original.jpg'
		},
		cover_image: null
	}
];

/** Empty history — the home page should render the "no history yet" empty state. */
export const emptyHistory: unknown[] = [];

/** App info fixture — minimal shape `getAppInfo` consumers need. */
export const appInfo = {
	version: '0.1.0-test',
	ani_cli_path: '/usr/bin/ani-cli',
	history_path: '/tmp/ani-hsts',
	proxy_base_url: 'http://127.0.0.1:0'
};

/** Default settings — used by /api/settings stubs. */
export const defaultSettings = {
	mode: 'sub',
	quality: 'best',
	auto_play_next: false,
	auto_skip_intro: false,
	auto_skip_outro: false,
	subtitle_track_index: 0,
	external_player: '',
	external_player_kind: 'Mpv',
	external_player_custom_args: '',
	syncplay_binary: 'syncplay',
	download_dir: '',
	download_bottom_bar_enabled: true,
	locale: 'en',
	last_update_dismissed_version: null
};

/**
 * Continue Watching fixture — a 12-episode show watched up through
 * episode 5. Used by the home-continue acceptance suite to assert
 * the card displays episode 6 (last+1) and clicking it routes a
 * `play` IPC with `episode=6`. The `id` doubles as the allmanga
 * show_id (resolveHistoryEntry maps HistoryEntry.id → ResumeTarget
 * .allmangaShowId), so the short-circuit through `/api/allmanga-
 * kitsu-map/<id>` resolves the match without exercising the live
 * kitsuSearch path.
 */
export const continueHistory = [
	{
		id: 'allmanga-test-show',
		ep_no: '5',
		title: 'Continue Test Show'
	}
];

/** Single-row history at the show's announced cap — pickNextEpisode
 *  must clamp to last (replay) rather than overshoot. */
export const atCapHistory = [
	{
		id: 'allmanga-test-show',
		ep_no: '12',
		title: 'Continue Test Show'
	}
];

/** Single-row history for an unresolvable show — `/api/allmanga-
 *  kitsu-map` returns null, the title-match cache misses, and
 *  `/api/kitsu/search` returns no hits. The Continue card should
 *  render as a /search-fallback link rather than a button. */
export const orphanHistory = [
	{
		id: 'allmanga-orphan-show',
		ep_no: '3',
		title: 'Orphan Test Show'
	}
];

/** The KitsuAnimeRef the allmanga-kitsu-map short-circuit resolves
 *  to. 12 episodes, finished — the simple case for at-cap clamping. */
export const continueKitsuMatch = {
	id: 'kitsu-continue',
	slug: 'continue-test-show',
	canonical_title: 'Continue Test Show',
	english_title: 'Continue Test Show',
	titles: { en_jp: 'Continue Test Show' },
	synopsis: 'Test show used by the home-continue acceptance suite.',
	episode_count: 12,
	average_rating: '70.00',
	start_date: '2024-01-12',
	status: 'finished',
	subtype: 'TV',
	poster_image: {
		tiny: 'https://media.kitsu.app/anime/poster_images/continue/tiny.jpg',
		small: 'https://media.kitsu.app/anime/poster_images/continue/small.jpg',
		medium: 'https://media.kitsu.app/anime/poster_images/continue/medium.jpg',
		large: 'https://media.kitsu.app/anime/poster_images/continue/large.jpg',
		original: 'https://media.kitsu.app/anime/poster_images/continue/original.jpg'
	},
	cover_image: null
};

/** Episode metadata for episode 6 of the continue test show. */
export const continueKitsuEpisode6 = {
	id: 'ep-6',
	number: 6,
	relative_number: 6,
	canonical_title: 'Episode Six',
	thumbnail: {
		tiny: null,
		small: null,
		large: null,
		original: 'https://media.kitsu.app/episodes/6/thumb.jpg'
	}
};
