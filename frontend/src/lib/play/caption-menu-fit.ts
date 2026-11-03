/**
 * The captions menu inside the player: how tall it may grow, where it
 * scrolls to show the selected entry, and where the arrow keys move.
 *
 * The menu opens upward from the captions button. With a provider
 * that lists thirty-odd languages it ran past the top of the player
 * and under the app's top bar, so its height is capped at the room
 * between the player's top edge — or the top bar, where the page has
 * scrolled the player under it — and the button, and the menu
 * scrolls.
 * The play page measures the two edges when the menu opens and on
 * every resize while it is open, and hands them here.
 */

/** Edges in viewport pixels; gaps in pixels. */
export interface MenuRoom {
	/** The player's top edge. */
	frameTop: number;
	/** The captions button's top edge, which the menu sits above. */
	anchorTop: number;
	/** The space between the menu and the button. */
	gap: number;
	/** The space kept between the menu and the player's top edge. */
	margin: number;
	/** The lowest edge above the player that covers it — the app's
	 *  sticky top bar, which the page can scroll the player under. */
	ceiling?: number;
	/** The least height the menu keeps, however little room there is. */
	floor?: number;
}

/** The menu's greatest height, in whole pixels: the room below the
 *  player's top edge, or the top bar where that covers it, and never
 *  less than `floor`. */
export function menuHeight({
	frameTop,
	anchorTop,
	gap,
	margin,
	ceiling = -Infinity,
	floor = 0
}: MenuRoom): number {
	const top = Math.max(frameTop, ceiling);
	return Math.max(floor, Math.floor(anchorTop - gap - top - margin));
}

/** A scroll position and an entry's place, in the menu's own pixels. */
export interface ScrollView {
	scrollTop: number;
	viewHeight: number;
	itemTop: number;
	itemHeight: number;
}

/** The scroll position that brings the entry wholly into view with
 *  the least movement; the current one when it is already in view. */
export function revealScroll({ scrollTop, viewHeight, itemTop, itemHeight }: ScrollView): number {
	if (itemTop < scrollTop) return itemTop;
	if (itemTop + itemHeight > scrollTop + viewHeight) return itemTop + itemHeight - viewHeight;
	return scrollTop;
}

/** The entry `key` moves focus to from `current` (-1 for none) among
 *  `count`, wrapping at the ends; `null` for a key the menu leaves to
 *  the browser, or an empty menu. */
export function menuKeyStep(key: string, current: number, count: number): number | null {
	if (count === 0) return null;
	switch (key) {
		case 'ArrowDown':
			return (current + 1) % count;
		case 'ArrowUp':
			return current <= 0 ? count - 1 : current - 1;
		case 'Home':
			return 0;
		case 'End':
			return count - 1;
		default:
			return null;
	}
}
