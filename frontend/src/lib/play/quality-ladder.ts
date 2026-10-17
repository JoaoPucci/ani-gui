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

/** A level the viewer pinned, as the next stream is matched against
 *  it: the height its playlist lists, or — for a level that lists
 *  none — its rank from the top of the ladder (by height, then by
 *  bandwidth, so among levels that list no height, by bandwidth). */
export type QualityPinChoice = number | { rank: number };

/** The level a pin choice matches on a stream: the level of that
 *  height or the nearest below it, or the level of that rank from the
 *  top — the lowest past the end. */
export function pinnedLevel(pin: QualityPinChoice, levels: readonly LadderLevel[]): number {
	if (typeof pin === 'number') return targetLevel({ kind: 'height', height: pin }, levels);
	const ladder = ladderOrder(levels);
	return ladder[Math.max(0, ladder.length - 1 - pin.rank)] ?? -1;
}

/** The setting's target. Anything that is neither `worst` nor a
 *  height reads as `best`, the backend's own default. */
export function parseQualitySetting(setting: string | null | undefined): QualityTarget {
	if (setting === 'worst') return { kind: 'worst' };
	if (setting && /^[1-9]\d*$/.test(setting)) return { kind: 'height', height: Number(setting) };
	return { kind: 'best' };
}

/** The levels' indices, lowest first: by height, then by bitrate. */
export function ladderOrder(levels: readonly LadderLevel[]): number[] {
	return levels
		.map((_, i) => i)
		.sort((a, b) => levels[a].height - levels[b].height || levels[a].bitrate - levels[b].bitrate);
}

/** The level a target chooses: the highest for `best`, the lowest for
 *  `worst`, and for a height the highest level at or below it — the
 *  lowest when every level is above. -1 when there are no levels. */
export function targetLevel(target: QualityTarget, levels: readonly LadderLevel[]): number {
	const ladder = ladderOrder(levels);
	if (ladder.length === 0) return -1;
	if (target.kind === 'best') return ladder[ladder.length - 1];
	if (target.kind === 'worst') return ladder[0];
	const fits = ladder.filter((i) => levels[i].height <= target.height);
	return fits.length > 0 ? fits[fits.length - 1] : ladder[0];
}

/** Where a level removed from `oldLadder` re-seats: the nearest level
 *  below it that `mapping` keeps — or, with none below, the nearest
 *  above — as its new index, with whether it lies above. Null when no
 *  level is kept. */
export function survivorOf(
	oldLadder: readonly number[],
	mapping: readonly number[],
	level: number
): { level: number; above: boolean } | null {
	const rank = oldLadder.indexOf(level);
	for (const direction of [-1, 1] as const) {
		for (let r = rank + direction; r >= 0 && r < oldLadder.length; r += direction) {
			const kept = mapping[oldLadder[r]] ?? -1;
			if (kept >= 0) return { level: kept, above: direction === 1 };
		}
	}
	return null;
}

/** Whether `a` sits above `b` on a ladder. */
export function isAbove(ladder: readonly number[], a: number, b: number): boolean {
	return ladder.indexOf(a) > ladder.indexOf(b);
}

/** The nearest level above (`1`) or below (`-1`) `level` on a ladder
 *  that is not among `unavailable`; null when there is none. */
export function neighbourLevel(
	ladder: readonly number[],
	unavailable: ReadonlySet<number>,
	level: number,
	direction: 1 | -1
): number | null {
	for (let r = ladder.indexOf(level) + direction; r >= 0 && r < ladder.length; r += direction) {
		if (!unavailable.has(ladder[r])) return ladder[r];
	}
	return null;
}
