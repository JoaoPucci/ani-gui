/**
 * The quality setting read against a stream's levels.
 *
 * The setting (`best`, `worst`, or a height such as `1080`) chooses
 * the stream the backend resolves: a height or `worst` asks it for
 * that one rendition, and `best` keeps the master playlist, which
 * lists every level. Once a master reaches the player — `best`, or a
 * height the master does not serve, which the backend answers with
 * the master too — the level it plays is the player's choice, and
 * this is what the setting asks of that choice.
 */

/** What the setting asks of a stream's levels. */
export type QualityTarget =
	| { kind: 'best' }
	| { kind: 'worst' }
	| { kind: 'height'; height: number };

/** One level as far as choosing between levels goes: its picture's
 *  height — zero when the playlist names none — and its bitrate. */
export interface LadderLevel {
	readonly height: number;
	readonly bitrate: number;
}

/** The setting's target. Anything that is neither `worst` nor a
 *  height reads as `best`, the backend's own default. */
export function parseQualitySetting(setting: string | null | undefined): QualityTarget {
	void setting;
	return { kind: 'best' };
}

/** The levels' indices, lowest first: by height, then by bitrate. */
export function ladderOrder(levels: readonly LadderLevel[]): number[] {
	return levels.map((_, i) => i);
}

/** The level a target chooses: the highest for `best`, the lowest for
 *  `worst`, and for a height the highest level at or below it — the
 *  lowest when every level is above. -1 when there are no levels. */
export function targetLevel(target: QualityTarget, levels: readonly LadderLevel[]): number {
	void target;
	void levels;
	return -1;
}
