/**
 * The level the viewer pinned, by the height its playlist lists, for
 * the rest of the app's run: the next episode, and a stream recovered
 * in its place, start at the same height — or the nearest below it —
 * rather than at the setting's. "Auto" clears it.
 */

import type { QualityGovernor } from './quality-governor';
import type { QualityTrack } from './quality-track';

let pinnedHeight: number | null = null;

export const qualityPin = {
	get: (): number | null => pinnedHeight,
	/** Keeps `height`; a level of no known height pins nothing. */
	set(height: number | null): void {
		if (height !== null && height > 0) pinnedHeight = height;
	},
	clear(): void {
		pinnedHeight = null;
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
	qualityPin.set(track.pinHeight(choice));
	return `pinned ${track.describe(choice)}`;
}
