/**
 * Decide whether the play page's episode strip should paginate to
 * follow an episode change (prev/next buttons, auto-play-next).
 *
 * The strip shows a fixed page of tiles and opens on the page holding
 * the current episode; episode switches replace the URL on the same
 * route, so the page never remounts and the strip has to follow the
 * episode on its own. It follows only when it was showing the page
 * the episode just left — a strip the user paginated away to browse
 * is theirs, and yanking it back under auto-play would be the
 * timer-driven carousel behaviour the design rules forbid.
 */
export interface StripFollowInput {
	/** Episode that was playing before the URL changed. */
	prevEpisode: number;
	/** Episode now playing, from the fresh URL. */
	episode: number;
	/** UI page the strip currently displays (1-based). */
	currentPage: number;
	/** Tiles per strip page. */
	pageSize: number;
}

export type StripFollowDecision = { follow: true; page: number } | { follow: false };

export function decideStripFollow(input: StripFollowInput): StripFollowDecision {
	void input;
	return { follow: false };
}
