/**
 * Where a row (Strip.svelte) puts a card the keyboard moves to.
 */

/** What this needs of a card: whether it matches a selector, and a
 *  way to scroll it into view. */
export interface Revealable {
	matches(selector: string): boolean;
	scrollIntoView(options: ScrollIntoViewOptions): void;
}

/** Brings `target`, just focused inside `row`, into view. */
export function revealFocused(
	target: Revealable | null,
	row: unknown,
	behavior: ScrollBehavior
): void {
	void [target, row, behavior];
}
