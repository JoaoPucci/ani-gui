/**
 * The level the viewer pinned, by the height its playlist lists — or,
 * for a level that lists none, by its rank from the ladder's top — for
 * the rest of the app's run: the next episode, and a stream recovered
 * in its place, start at the same height or the nearest below it, or
 * at the same rank, rather than at the setting's. "Auto" clears it.
 */

import type { QualityGovernor } from './quality-governor';
import type { QualityPinChoice } from './quality-ladder';
import type { QualityTrack } from './quality-track';

let pinned: QualityPinChoice | null = null;

export const qualityPin = {
	/** The pinned height; null when nothing, or a rank, is pinned. */
	get: (): number | null => (typeof pinned === 'number' ? pinned : null),
	/** What the next stream is matched against. */
	choice: (): QualityPinChoice | null => pinned,
	/** Keeps a height, or a rank for a level that lists none. */
	set(choice: QualityPinChoice): void {
		pinned = choice;
	},
	clear(): void {
		pinned = null;
	}
};

/** Keeps what a pick in the menu left in force, and names it for the
 *  log: "auto", the level the governor pinned, or null for a pick it
 *  refused. */
export function rememberPick(
	choice: 'auto' | number,
	governor: QualityGovernor,
	track: QualityTrack
): string | null {
	if (choice === 'auto') {
		qualityPin.clear();
		return 'auto';
	}
	if (!governor.isPinned || governor.level !== choice) return null;
	qualityPin.set(track.pinFor(choice));
	return `pinned ${track.describe(choice)}`;
}
