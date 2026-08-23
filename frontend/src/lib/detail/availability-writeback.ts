/**
 * Who owns each part of a show's availability row while more than one
 * answer is in flight.
 *
 * Two questions can be out at once: the one a page asks on load, which
 * reads through the cache, and the one a click on a dimmed tile asks,
 * which skips it. The re-ask wins where they disagree — it is newer and
 * it went to the provider on purpose.
 *
 * But a re-ask does not always answer everything. When the provider only
 * offers the count off the search hit, that number counts halves as
 * whole episodes, so the re-ask reports the show as listed and reports
 * nothing about the cap. Treating that as a whole answer discards the
 * ordinary lookup wholesale, confirmed count and all, and leaves the
 * page on the count the row held before — which after a mode change
 * belongs to the other catalogue (Codex P2 #3674622194).
 *
 * So ownership is per field. A re-ask claims the verdict always and the
 * count only when it confirmed one; whatever it left open, a lookup
 * still out may fill.
 */

import type { CapGateRefresh } from './cap-gate-probe';

/** What an ordinary, cache-reading lookup came back with. */
export interface AvailabilityAnswer {
	available: boolean;
	count: number | null;
	extraEpisodes: string[];
}

/**
 * The fields the caller may write. Absent keys are not "unknown" — they
 * belong to someone else, and the page keeps what it has.
 */
export interface AvailabilityPatch {
	available?: boolean;
	count?: number | null;
	extraEpisodes?: string[];
}

export interface AvailabilityWriteback {
	/**
	 * Taken before a lookup's request goes out. The returned function
	 * settles it, reporting what is still this lookup's to write.
	 */
	begin: () => (answer: AvailabilityAnswer) => AvailabilityPatch;
	/** A cache-skipping re-ask answered. */
	refresh: (refresh: CapGateRefresh) => AvailabilityPatch;
}

/**
 * `currentContext` names the row being asked about — show and mode
 * together, since the provider catalogues sub and dub separately and a
 * count from one says nothing about the other. A lookup that settles
 * against a different context answers a question nobody is asking any
 * more, so it writes nothing.
 */
export function createAvailabilityWriteback(currentContext: () => string): AvailabilityWriteback {
	let verdicts = 0;
	let counts = 0;
	return {
		begin() {
			const asked = currentContext();
			const verdictsAtStart = verdicts;
			const countsAtStart = counts;
			return (answer: AvailabilityAnswer) => {
				if (currentContext() !== asked) return {};
				const patch: AvailabilityPatch = {};
				const verdictStands = verdicts === verdictsAtStart;
				if (verdictStands) patch.available = answer.available;
				// A negative answer's count is not a measurement — it is
				// the verdict restated, so it is superseded with it. Left
				// in, it publishes a null cap on a show a re-ask has just
				// found, and both routes read a null cap as unbounded.
				// A positive answer counted episodes, and that stands on
				// its own however the verdict fared.
				if (counts === countsAtStart && (verdictStands || answer.available)) {
					patch.count = answer.count;
					patch.extraEpisodes = answer.extraEpisodes;
				}
				return patch;
			};
		},
		refresh(refresh: CapGateRefresh) {
			verdicts += 1;
			const patch: AvailabilityPatch = { available: refresh.available };
			if (refresh.count != null && refresh.extraEpisodes != null) {
				counts += 1;
				patch.count = refresh.count;
				patch.extraEpisodes = refresh.extraEpisodes;
			}
			return patch;
		}
	};
}
