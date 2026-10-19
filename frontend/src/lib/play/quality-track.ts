/**
 * What one stream's levels are known to be: the playlist's heights and
 * bitrates, the height each level's segments turned out to be encoded
 * at once one was parsed, and the level that plays. It names levels
 * for the log and builds the view the menu reads.
 */

import type { QualityGovernor, QualityMove } from './quality-governor';
import { ladderOrder, type LadderLevel, type QualityPinChoice } from './quality-ladder';
import type { QualityView } from './quality-menu';
import { isMain } from './quality-rendition';
import { describeLevel, describeMove } from './quality-names';
import { levelMapping, remapIndex, remapKeys } from './quality-remap';

/** Whether two sets of the same levels sort into the same ladder. */
function sameOrder(a: readonly LadderLevel[], b: readonly LadderLevel[]): boolean {
	return ladderOrder(a).join() === ladderOrder(b).join();
}

export class QualityTrack {
	private playlist: LadderLevel[] = [];
	/** The engine's level objects, which a removal is matched by. */
	private objects: readonly LadderLevel[] = [];
	private encoded = new Map<number, number>();
	private playing: number | null = null;

	/** The master's levels arrived. */
	arrive(levels: readonly LadderLevel[]): void {
		this.objects = levels;
		this.playlist = levels.map((l) => ({ height: l.height, bitrate: l.bitrate }));
		this.encoded.clear();
		this.playing = null;
	}

	/** The engine's level objects as last known. */
	get engineLevels(): readonly LadderLevel[] {
		return this.objects;
	}

	/** hls.js removed levels: the heights learned and the level playing
	 *  are carried to the levels left; the mapping comes back. */
	remap(levels: readonly LadderLevel[]): number[] {
		const mapping = levelMapping(this.objects, levels);
		this.objects = levels;
		this.playlist = levels.map((l) => ({ height: l.height, bitrate: l.bitrate }));
		this.encoded = remapKeys(this.encoded, mapping);
		this.playing = remapIndex(this.playing, mapping);
		return mapping;
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
		return describeLevel(this.levels(), this.playlist, level);
	}

	/** A move as the log names it. */
	describeMove(move: QualityMove): string {
		return describeMove((level) => this.describe(level), move);
	}

	/** What to keep for a pin of `level`: the height its playlist
	 *  lists, which the next stream's levels are matched by, or — when
	 *  it lists none — its rank from the top of the ladder. */
	pinFor(level: number): QualityPinChoice {
		const { height } = this.playlist[level];
		if (height > 0) return height;
		const ladder = ladderOrder(this.playlist);
		return { rank: ladder.length - 1 - ladder.indexOf(level) };
	}

	/** A segment started playing; true when its level is a change. */
	notePlaying(frag: { type?: unknown; level: number }): boolean {
		// A level outside the stream's — hls.js marks a removed current
		// level's segments -1 — names nothing.
		const known = frag.level >= 0 && frag.level < this.playlist.length;
		if (!isMain(frag) || !known || frag.level === this.playing) return false;
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

	/** The level whose segment plays; null before one plays. */
	get playingLevel(): number | null {
		return this.playing;
	}

	/** The levels as their playlists declare them. The policy — the
	 *  governor's ladder and ceiling, and a pin matched against the next
	 *  stream — reads these; only the menu and the log read `levels()`,
	 *  with the heights segments turned out to be encoded at. */
	declared(): LadderLevel[] {
		return this.playlist.map((l) => ({ ...l }));
	}

	/** What the menu shows. */
	view(governor: QualityGovernor): QualityView {
		return {
			levels: this.levels(),
			pinned: governor.pinnedLevel,
			playing: this.playing,
			unavailable: governor.unavailableLevels,
			// The rows in the order the player moves through the levels,
			// when the encoded heights would sort them otherwise.
			order: sameOrder(this.levels(), this.playlist) ? undefined : ladderOrder(this.playlist)
		};
	}
}
