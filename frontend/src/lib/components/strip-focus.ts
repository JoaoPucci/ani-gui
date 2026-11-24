/**
 * Where a row (Strip.svelte) puts a card the keyboard moves to.
 *
 * Chromium scrolls a focused element into view only when it is out of
 * sight: a card the row's edge cuts across stays where it is, and its
 * focus ring is cut with it. `nearest` moves a card just far enough to
 * show it whole, clear of the row's scroll padding, and leaves one
 * already in view where it is.
 */

/** What this needs of a card: whether it matches a selector, and a
 *  way to scroll it into view. */
export interface Revealable {
	matches(selector: string): boolean;
	scrollIntoView(options: ScrollIntoViewOptions): void;
}

/** Brings `target`, just focused inside `row`, into view when the
 *  keyboard focused it. A click leaves the card where it was clicked,
 *  and the row taking focus itself moves nothing. */
export function revealFocused(
	target: Revealable | null,
	row: unknown,
	behavior: ScrollBehavior
): void {
	if (!target || target === row || !target.matches(':focus-visible')) return;
	target.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior });
}
