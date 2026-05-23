/**
 * Pure logic for the detail-page primary-action CTA label. The
 * Svelte derived value in routes/anime/[id]/+page.svelte maps the
 * returned state to one of five `m.detail_play_button_*()` i18n
 * messages.
 */

/**
 * Single-video detection. True for shows whose detail page should
 * never read "Play episode 1":
 *
 *   - Kitsu subtype `"movie"` always — a movie is one video by
 *     definition, regardless of episode_count or airing status.
 *   - Other subtypes (`OVA`, `special`, `ONA`, etc.) only when
 *     `episodeCount === 1` AND `status === "finished"`.
 *
 * Only `finished` guarantees `episodeCount` is final. The other
 * non-current values (`upcoming` / `tba` / `unreleased`) describe
 * pre-airing states where Kitsu's count is an announcement, not a
 * confirmed total — a future multi-episode OVA could be listed
 * with just one announced episode today and mis-label as a single-
 * video work. `current` has the same problem with an aired-so-far
 * count. Movies bypass all of this via the first branch (`movie`
 * is one video by definition, even before release).
 *
 * Failure mode of this strict rule: a not-yet-finished single-
 * episode OVA reads as "Play episode 1" until Kitsu marks it
 * finished. That's the right tradeoff — TV-style copy works on
 * any show; "Watch" mislabels a TV show.
 *
 * Inputs are typed as `string | null | undefined` because Kitsu
 * fields commonly arrive as `Option<String>` from the backend and
 * flow into the frontend's `detail` view-model unchanged.
 */
export function isSingleVideo(
	subtype: string | null | undefined,
	episodeCount: number | null | undefined,
	status: string | null | undefined
): boolean {
	if (subtype === 'movie') return true;
	if (episodeCount === 1 && status === 'finished') return true;
	return false;
}

/**
 * State returned by {@link computePlayLabel}. The five kinds map
 * to:
 *
 *   - `watch` — single-video show, never started → "Watch"
 *   - `watch_again` — single-video show, history exists → "Watch again"
 *   - `episode_one` — multi-ep show, never started → "Play episode 1"
 *   - `resume` — multi-ep show, continuing past last watched → "Continue · Episode N"
 *   - `replay` — multi-ep show, last episode reached → "Replay · Episode N"
 */
export type PlayLabelState =
	| { kind: 'watch' }
	| { kind: 'watch_again' }
	| { kind: 'episode_one' }
	| { kind: 'resume'; episode: number }
	| { kind: 'replay'; episode: number };

/**
 * Resolve the CTA state from the three inputs the detail page has
 * in scope when the user is about to click play.
 *
 *   - `isSingleVideo` is the boolean from {@link isSingleVideo}.
 *   - `resumeEntry` is the ani-cli history row for this show, or
 *     null when the user has never played it.
 *   - `defaultEpisode` is the episode the play button would
 *     dispatch to — already capped at the show's known
 *     `episode_count` by the page's `defaultEpisode()` derivation.
 *
 * Single-video shows ignore `defaultEpisode` entirely: with or
 * without a resume entry, there's only one video, so the CTA
 * reads "Watch" or "Watch again" — never with an episode number.
 */
export function computePlayLabel(args: {
	isSingleVideo: boolean;
	resumeEntry: { ep_no: string } | null | undefined;
	defaultEpisode: number;
}): PlayLabelState {
	const { isSingleVideo, resumeEntry, defaultEpisode } = args;
	if (isSingleVideo) {
		return resumeEntry ? { kind: 'watch_again' } : { kind: 'watch' };
	}
	if (!resumeEntry) return { kind: 'episode_one' };
	const last = parseInt(resumeEntry.ep_no, 10);
	if (Number.isFinite(last) && defaultEpisode === last) {
		return { kind: 'replay', episode: defaultEpisode };
	}
	return { kind: 'resume', episode: defaultEpisode };
}
