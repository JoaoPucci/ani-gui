/**
 * The level the viewer pinned, as the governor keeps it: an index into
 * the stream's levels, kept apart from the level held, so the hold can
 * sit below it after a failure and climb back to it. A pin is never
 * exceeded while it is shown: whatever moves the hold or the levels
 * either keeps the pin at or above the hold, moves it down to a level
 * that can load, or lets it go.
 */

import {
	isAbove,
	isLoadable,
	neighbourLevel,
	pinnedLevel,
	survivorOf,
	type LadderLevel,
	type QualityPinChoice
} from './quality-ladder';

/** The level a run's pin starts a stream at, or null when no level is
 *  at or below the pinned height: the stream then starts as Auto. */
export function startPin(
	pin: QualityPinChoice | null,
	levels: readonly LadderLevel[]
): number | null {
	const level = pin === null ? -1 : pinnedLevel(pin, levels);
	return level >= 0 ? level : null;
}

/** The pin once `unavailable` may have grown: the pinned level while it
 *  can load, else the nearest loadable level below it, else none — a
 *  pin with nothing loadable at or below it is let go. */
export function demotePin(
	ladder: readonly number[],
	unavailable: ReadonlySet<number>,
	pin: number | null
): number | null {
	if (pin === null) return null;
	return isLoadable(ladder, unavailable, pin) ? pin : neighbourLevel(ladder, unavailable, pin, -1);
}

/** The pin after the engine was followed from `left` to `level` over a
 *  side rendition. Let go when the engine moved above it, where it
 *  cannot be honoured. Moved to `level` when the level left was the pin
 *  itself and the failure was its own (`blamed`): hls.js skips every
 *  level sharing the failing rendition's group, so the levels between
 *  would fail the same way. Kept otherwise — the pinned level's own
 *  rendition was not judged, whether the play sat below it or a
 *  removal had only just re-seated it there — and the play climbs back
 *  to it; demoted past a level that cannot load. */
export function followedPin(
	ladder: readonly number[],
	unavailable: ReadonlySet<number>,
	pin: number | null,
	left: number,
	level: number,
	blamed: boolean
): number | null {
	const kept = demotePin(ladder, unavailable, pin);
	if (kept === null || isAbove(ladder, level, kept)) return null;
	return blamed && left === pin ? level : kept;
}

/** The pin carried across hls.js removing levels: its own level's new
 *  index, or the nearest loadable survivor below it, or none. */
export function remapPin(
	pin: number | null,
	mapping: readonly number[],
	oldLadder: readonly number[],
	unavailable: ReadonlySet<number>
): number | null {
	if (pin === null) return null;
	const kept = mapping[pin] ?? -1;
	if (kept >= 0) return kept;
	const seat = survivorOf(oldLadder, mapping, pin, unavailable);
	return seat?.above === false ? seat.level : null;
}

/** Whether a pin still bounds the hold: there is one, and the hold is
 *  not above it. */
export function pinHolds(ladder: readonly number[], pin: number | null, current: number): boolean {
	return pin !== null && !isAbove(ladder, current, pin);
}
