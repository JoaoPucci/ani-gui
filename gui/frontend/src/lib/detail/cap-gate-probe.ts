import { beyondPlayable } from './episode-caps';

/**
 * Re-asking availability when a user clicks a cap-gated episode tile.
 *
 * A tile is cap-gated when the anime database says the episode aired
 * but it sits above the episode count allmanga reported — the
 * catalog-lag case `beyondPlayable` describes. That count is a
 * snapshot, cached for 24 hours on an ongoing show, and allmanga adds
 * episodes inside that window. So the tile can stay dead for most of
 * a day after the episode became streamable, and until now the click
 * returned early: no request, no message, nothing to distinguish it
 * from a broken control.
 *
 * The re-ask must therefore set `bypass_cache`. Without it the lookup
 * answers from the very row being questioned, confirms the tile's own
 * claim, and reaches nobody — which is what the first version of this
 * did. The interactive lane matters too: the scraper gate paces and
 * refuses background traffic but never a request a user is waiting
 * on.
 *
 * ONE LOOKUP PER SHOW. "How many episodes do you have?" does not vary
 * by episode, so three dimmed tiles clicked in a row are the same
 * question three times at a rate-limited site. The first click sends
 * it, later clicks join, and every waiting tile is judged against the
 * single answer. Sharing lasts only while the request is in flight —
 * a click afterwards deserves a current answer rather than a replay.
 *
 * Outcomes, and why each is what it is:
 *
 *   - The fresh count reaches the episode → play it. The click
 *     already expressed the intent; the stale count was the obstacle.
 *   - It still falls short, the lookup produced no count, or it
 *     failed → say so. Silence is indistinguishable from the dead
 *     tile this exists to replace.
 */
export interface CapGateProbeDeps {
	/** Ask allmanga how many episodes it has for THIS SHOW, skipping
	 *  the cached row. Resolves to the count, or null when it has
	 *  none. Takes no episode: the question is show-level. */
	probe: () => Promise<number | null>;
	/** The fresh count reaches the episode — publish it and play. */
	onCleared: (episode: number, count: number) => void;
	/** Still short, countless, or failed. */
	onStillGated: (episode: number) => void;
}

export function createCapGateProbe(deps: CapGateProbeDeps): {
	request: (episode: number) => void;
	isProbing: (episode: number) => boolean;
} {
	/** The shared in-flight lookup, or null when none is out. */
	let pending: Promise<number | null> | null = null;
	/** Tiles waiting on it — per episode, because the spinner belongs
	 *  to the tile the user pressed rather than to the whole strip. */
	const waiting = new Set<number>();

	return {
		isProbing: (episode) => waiting.has(episode),
		request: (episode) => {
			if (waiting.has(episode)) return;
			waiting.add(episode);
			// Cleared as soon as the request settles, so a click after
			// this one gets a current answer instead of joining a
			// lookup that has already finished.
			pending ??= deps.probe().finally(() => {
				pending = null;
			});
			void pending
				.then((count) => {
					// `beyondPlayable` decides, rather than a fresh
					// comparison: the rule that dimmed the tile has to be
					// the rule that un-dims it, half-episode floor-compare
					// included.
					if (count != null && !beyondPlayable(episode, count)) {
						deps.onCleared(episode, count);
					} else {
						deps.onStillGated(episode);
					}
				})
				.catch(() => deps.onStillGated(episode))
				// Released on every path. A rejection that left the tile
				// marked would swallow its later clicks as duplicates and
				// wedge it shut for good.
				.finally(() => waiting.delete(episode));
		}
	};
}
