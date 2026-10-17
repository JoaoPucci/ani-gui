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

import { m } from '$lib/paraglide/messages';
import { getLocale } from '$lib/paraglide/runtime';
import { ladderOrder, type LadderLevel } from './quality-ladder';

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
	/** The levels in the order the player moves through them — by the
	 *  heights their playlists declare — when that differs from the
	 *  order of the heights named here. */
	readonly order?: readonly number[];
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
	const { height, bitrate } = levels[index];
	const mbps = new Intl.NumberFormat(getLocale(), {
		minimumFractionDigits: 1,
		maximumFractionDigits: 1
	}).format(bitrate / 1_000_000);
	if (height <= 0) return m.play_quality_bitrate({ mbps });
	const twin = levels.some((other, i) => i !== index && other.height === height);
	return twin
		? m.play_quality_height_bitrate({ height: String(height), mbps })
		: m.play_quality_height({ height: String(height) });
}

/** The menu, or null when there is nothing to choose: no stream, or a
 *  stream of one level. */
export function qualityMenu(view: QualityView | null): QualityMenu | null {
	if (!view || view.levels.length < 2) return null;
	const { levels, pinned, playing, unavailable } = view;
	const offered = [...(view.order ?? ladderOrder(levels))]
		.reverse()
		.filter((i) => !unavailable.includes(i));
	const items: QualityMenuItem[] = [
		{ key: 'auto', label: m.play_quality_auto(), checked: pinned === null },
		...offered.map((i) => ({ key: i, label: levelLabel(levels, i), checked: pinned === i }))
	];
	if (pinned !== null) return { items, current: levelLabel(levels, pinned) };
	const plays = playing !== null && playing < levels.length;
	const current = plays
		? m.play_quality_auto_playing({ quality: levelLabel(levels, playing) })
		: m.play_quality_auto();
	return { items, current };
}
