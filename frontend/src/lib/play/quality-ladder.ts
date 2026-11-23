/**
 * A stream's levels as the player chooses among them.
 *
 * The backend resolves the master for every play, so the level a
 * stream plays is the player's choice: Auto starts at the highest, a
 * pick in the menu pins one (`./quality-pin`), and the hold never
 * climbs above its target (`./quality-hold`).
 */

/** What a hold aims at among a stream's levels: the highest (Auto),
 *  the lowest, or a height — the level of that height, or the nearest
 *  below it. */
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
 *  top — the lowest past the end. -1 when no level is at or below a
 *  pinned height. */
export function pinnedLevel(pin: QualityPinChoice, levels: readonly LadderLevel[]): number {
	if (typeof pin === 'number') {
		// At the height or below it: a pin is never exceeded, so with
		// every level above it there is none.
		const level = targetLevel({ kind: 'height', height: pin }, levels);
		return (levels[level]?.height ?? Infinity) <= pin ? level : -1;
	}
	const ladder = ladderOrder(levels);
	return ladder[Math.max(0, ladder.length - 1 - pin.rank)] ?? -1;
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
 *  below it that `mapping` keeps and can load — or, with none, the nearest
 *  above — as its new index, with whether it lies above. Null when no
 *  level is kept. */
export function survivorOf(
	oldLadder: readonly number[],
	mapping: readonly number[],
	level: number,
	unavailable: ReadonlySet<number>
): { level: number; above: boolean } | null {
	const rank = oldLadder.indexOf(level);
	for (const direction of [-1, 1] as const) {
		for (let r = rank + direction; r >= 0 && r < oldLadder.length; r += direction) {
			const kept = mapping[oldLadder[r]] ?? -1;
			if (kept >= 0 && !unavailable.has(kept)) return { level: kept, above: direction === 1 };
		}
	}
	return null;
}

/** Whether `level` is on the ladder and not among `unavailable`. */
export function isLoadable(
	ladder: readonly number[],
	unavailable: ReadonlySet<number>,
	level: number
): boolean {
	return ladder.includes(level) && !unavailable.has(level);
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
