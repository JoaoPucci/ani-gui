import { beyondPlayable } from './episode-caps';

/**
 * Re-asking availability when a user clicks a cap-gated episode tile.
 *
 * A tile is cap-gated when AniList says the episode has aired but it
 * sits above the playable count allmanga reported — the catalog-lag
 * case `beyondPlayable` describes. The count is a snapshot taken when
 * the page mounted, and allmanga catches up during the visit, so the
 * tile can be dead for an episode that became streamable minutes ago.
 * Until now the click handler simply returned early, which reads as a
 * broken tile rather than as a deliberate state.
 *
 * The re-ask runs in the INTERACTIVE lane. That is the whole reason a
 * click can succeed where the mount-time probe did not: the scraper
 * gate paces and refuses background traffic but never a request a
 * user is waiting on.
 *
 * Outcomes, and why each is what it is:
 *
 *   - The fresh count reaches the episode → play it. The click
 *     already expressed the intent; the stale cap was the obstacle.
 *   - It still falls short, the probe could not produce a count, or
 *     it failed outright → say so. Silence here is indistinguishable
 *     from the dead tile this exists to replace.
 *
 * One request per episode at a time, so an impatient user cannot turn
 * a single dimmed tile into a burst of interactive scraper traffic.
 * Different episodes are independent: dimming one tile must not dim
 * its neighbours.
 */
export interface CapGateProbeDeps {
	/** Interactive availability lookup for one episode's show.
	 *  Resolves to the playable count, or null when it has none. */
	probe: (episode: number) => Promise<number | null>;
	/** The fresh count reaches the episode — publish it and play. */
	onCleared: (episode: number, count: number) => void;
	/** Still short, countless, or failed. */
	onStillGated: (episode: number) => void;
}

export function createCapGateProbe(deps: CapGateProbeDeps): {
	request: (episode: number) => void;
	isProbing: (episode: number) => boolean;
} {
	const inFlight = new Set<number>();

	return {
		isProbing: (episode) => inFlight.has(episode),
		request: (episode) => {
			if (inFlight.has(episode)) return;
			inFlight.add(episode);
			void deps
				.probe(episode)
				.then((count) => {
					// `beyondPlayable` rather than a fresh comparison: the
					// rule that dimmed the tile has to be the rule that
					// un-dims it, half-episode floor-compare included.
					if (count != null && !beyondPlayable(episode, count)) {
						deps.onCleared(episode, count);
					} else {
						deps.onStillGated(episode);
					}
				})
				.catch(() => deps.onStillGated(episode))
				// Released on every path. A rejection that left the marker
				// set would swallow every later click as a duplicate and
				// wedge the tile shut for good.
				.finally(() => inFlight.delete(episode));
		}
	};
}
