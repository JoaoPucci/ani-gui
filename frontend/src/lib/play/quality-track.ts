/**
 * What one stream's levels are known to be: the playlist's heights and
 * bitrates, the height each level's segments turned out to be encoded
 * at once one was parsed, and the level that plays. It names levels
 * for the log and builds the view the menu reads.
 */

import type { QualityGovernor, QualityMove } from './quality-governor';
import { ladderOrder, type LadderLevel, type QualityPinChoice } from './quality-ladder';
import type { QualityView } from './quality-menu';
import { isMain } from './quality-report';

export class QualityTrack {
	private playlist: LadderLevel[] = [];
	private readonly encoded = new Map<number, number>();
	private playing: number | null = null;

	/** The master's levels arrived. */
	arrive(levels: readonly LadderLevel[]): void {
		this.playlist = levels.map((l) => ({ height: l.height, bitrate: l.bitrate }));
		this.encoded.clear();
		this.playing = null;
	}

	/** The levels, at the height each is known to be encoded at. */
	levels(): LadderLevel[] {
		return this.playlist.map((l, i) => ({
			height: this.encoded.get(i) ?? l.height,
			bitrate: l.bitrate
		}));
	}

	/** A level as the log names it. */
	describe(level: number): string {
		const { height, bitrate } = this.levels()[level];
		const mbps = `${(bitrate / 1_000_000).toFixed(1)} Mbps`;
		return height > 0 ? `${height}p (level ${level}, ${mbps})` : `level ${level} (${mbps})`;
	}

	/** A move as the log names it. */
	describeMove(move: QualityMove): string {
		const from = move.from === null ? '' : `${this.describe(move.from)} → `;
		return `${move.reason}: ${from}${this.describe(move.level)}`;
	}

	/** What to keep for a pin of `level`: the height its playlist
	 *  lists, which the next stream's levels are matched by, or — when
	 *  it lists none — its rank by bandwidth from the top. */
	pinFor(level: number): QualityPinChoice {
		const { height } = this.playlist[level];
		if (height > 0) return height;
		const ladder = ladderOrder(this.playlist);
		return { rank: ladder.length - 1 - ladder.indexOf(level) };
	}

	/** A segment started playing; true when its level is a change. */
	notePlaying(frag: { type?: unknown; level: number }): boolean {
		if (!isMain(frag) || frag.level === this.playing) return false;
		this.playing = frag.level;
		return true;
	}

	/** A level turned out to be encoded at a height: the news, with the
	 *  height its playlist listed, or null when it is no news. */
	noteEncoded(
		seen: { level: number; height: number } | null
	): { level: number; height: number; listed: number } | null {
		const listed = seen ? this.playlist[seen.level]?.height : undefined;
		if (!seen || listed === undefined || this.encoded.get(seen.level) === seen.height) return null;
		this.encoded.set(seen.level, seen.height);
		return { ...seen, listed };
	}

	/** What the menu shows. */
	view(governor: QualityGovernor): QualityView {
		return {
			levels: this.levels(),
			pinned: governor.isPinned ? governor.level : null,
			playing: this.playing,
			unavailable: governor.unavailableLevels
		};
	}
}
