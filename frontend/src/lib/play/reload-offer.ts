/**
 * The episodes a play cannot reach, as the player knows them.
 *
 * Every play path on the player — tile clicks, prev/next,
 * auto-play-next, and the Reload on the player-error overlay, which
 * re-resolves the episode on screen — goes through one switch, and
 * the switch refuses an episode in one of these states before it asks
 * the provider anything:
 *
 *   • `delisted` — the provider no longer lists the show in the
 *     chosen audio. A delisted show comes back without a count, so
 *     the cap check cannot catch it: the cap is whatever it already
 *     was.
 *   • `unaired` — the schedule says the episode has not aired.
 *   • `beyond-playable` — the episode is past the provider's playable
 *     count (catalogue lag).
 *
 * None of them is a state a retry leaves: each holds until the page
 * learns something new about the show.
 */

/** Why a play cannot reach the episode. */
export type DeadEnd = 'delisted' | 'unaired' | 'beyond-playable';

/** What the page knows about the episode a play asks for. */
export interface EpisodeStanding {
	/** Whether the provider lists the show in the chosen audio; `null`
	 *  until an answer lands. */
	listed: boolean | null;
	/** Whether the schedule calls the episode unaired. */
	unaired: boolean;
	/** Whether the episode is past the provider's playable count. */
	beyondPlayable: boolean;
}

/** The state that keeps a play from the episode, or `null` when
 *  nothing known does. An unknown listing claims nothing. */
export function deadEnd(s: EpisodeStanding): DeadEnd | null {
	if (s.listed === false) return 'delisted';
	if (s.unaired) return 'unaired';
	if (s.beyondPlayable) return 'beyond-playable';
	return null;
}
