/**
 * How the log names a level and a move. A level is named by the height
 * its segments are encoded at — what the viewer sees — and, when its
 * playlist declares another, by that too: the player moves through the
 * levels in the declared order, so a line naming only encoded heights
 * could read as a step in the wrong direction.
 */

import type { QualityMove } from './quality-hold';
import type { LadderLevel } from './quality-ladder';

export function describeLevel(
	levels: readonly LadderLevel[],
	declared: readonly LadderLevel[],
	level: number
): string {
	const { height, bitrate } = levels[level];
	const listed = declared[level]?.height ?? height;
	const also = listed > 0 && listed !== height ? `, listed ${listed}p` : '';
	const mbps = `${(bitrate / 1_000_000).toFixed(1)} Mbps`;
	return height > 0 ? `${height}p (level ${level}${also}, ${mbps})` : `level ${level} (${mbps})`;
}

export function describeMove(name: (level: number) => string, move: QualityMove): string {
	const from = move.from === null ? '' : `${name(move.from)} → `;
	return `${move.reason}: ${from}${name(move.level)}`;
}
