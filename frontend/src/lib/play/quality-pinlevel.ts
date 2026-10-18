/**
 * The level the viewer pinned, as the governor keeps it: an index into
 * the stream's levels, kept apart from the level held, so the hold can
 * sit below it after a failure and climb back to it.
 */

import { pinnedLevel, type LadderLevel, type QualityPinChoice } from './quality-ladder';

/** The level a run's pin starts a stream at, or null when no level is
 *  at or below the pinned height: the stream then starts as Auto. */
export function startPin(
	pin: QualityPinChoice | null,
	levels: readonly LadderLevel[]
): number | null {
	const level = pin === null ? -1 : pinnedLevel(pin, levels);
	return level >= 0 ? level : null;
}
