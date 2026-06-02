/**
 * Pick the episode the player should jump to when the user clicks
 * "Continue" — same rules across the home Continue Watching card and
 * the detail-page CTA so the two surfaces don't disagree on what
 * Continue means.
 *
 * Rules, in evaluation order:
 *
 *   1. No history (null) or unparsable / sub-1 lastWatched → 1.
 *      Detail page's "Play episode 1" branch. Home cards never hit
 *      this in practice (the card only renders for entries with
 *      history), but the helper has to answer to both call sites.
 *   2. next = lastWatched + 1.
 *   3. If a cap is known and next would exceed it → lastWatched
 *      (Replay branch). Single-video shows (movies, finished 1-ep
 *      OVAs) fall through here naturally: lastWatched=1, cap=1 →
 *      next=2 > 1, return 1. No separate isSingleVideo argument.
 *   4. Otherwise → next.
 *
 * Cap unknown (`episode_count` missing on the Kitsu ref): skip the
 * fence and advance. Kitsu omits episode_count for some ONA / upcoming
 * shows; without a cap we can't guard overshoot, but the surrounding
 * availability filter already drops streams that aren't resolvable,
 * and a phantom "episode 100" click would surface via the lazy click
 * path the same way an out-of-range manual navigation would.
 */
export function pickNextEpisode(lastWatched: number | null, episodeCap: number | null): number {
	if (lastWatched === null || !Number.isFinite(lastWatched) || lastWatched < 1) return 1;
	const next = lastWatched + 1;
	if (episodeCap !== null && next > episodeCap) return lastWatched;
	return next;
}
