/**
 * The player's quality menu: "Auto" and the stream's levels, highest
 * first, each named by its picture's height, with the choice in force
 * checked and the row's own value saying what plays.
 *
 * A level is named by the height its picture is encoded at once the
 * engine has parsed one of its segments, and by its playlist's
 * `RESOLUTION` until then — the two can differ, and the encoded one is
 * what the viewer sees.
 */

import type { LadderLevel } from './quality-ladder';

/** What the menu shows, as the player's quality control publishes it. */
export interface QualityView {
	/** Every level, at the height it is known to be encoded at. */
	readonly levels: readonly LadderLevel[];
	/** The level the viewer pinned; null while the choice is "Auto". */
	readonly pinned: number | null;
	/** The level of the segment playing; null before one plays. */
	readonly playing: number | null;
	/** Levels the engine could not load, left out of the menu. */
	readonly unavailable: readonly number[];
}

export interface QualityMenuItem {
	readonly key: 'auto' | number;
	readonly label: string;
	readonly checked: boolean;
}

export interface QualityMenu {
	readonly items: readonly QualityMenuItem[];
	/** The row's value: the pinned level, or "Auto" with what plays. */
	readonly current: string;
}

/** A level's name: its height, with its bitrate beside it when another
 *  level has the same height, and its bitrate alone when the playlist
 *  names no height. */
export function levelLabel(levels: readonly LadderLevel[], index: number): string {
	void levels;
	void index;
	return '';
}

/** The menu, or null when there is nothing to choose: no stream, or a
 *  stream of one level. */
export function qualityMenu(view: QualityView | null): QualityMenu | null {
	void view;
	return null;
}
