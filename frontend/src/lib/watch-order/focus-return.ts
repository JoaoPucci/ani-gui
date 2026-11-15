/**
 * Where the Up next card gives focus back when it is dismissed.
 *
 * The card never takes focus itself, so focus inside it came from
 * somewhere: the element it came from is kept while focus stays in the
 * card and forgotten when focus leaves it. A keyboard dismissal (Esc,
 * or Enter or Space on the close button) made while focus is in the
 * card gives focus back to that element. A pointer dismissal gives it
 * back nowhere, so a click cannot hand focus to a player control and
 * keep the fullscreen controls up.
 *
 * Pure over whatever stands for an element: the component passes DOM
 * elements and a containment test, and focuses the answer.
 */

/** The element kept after focus moved into the card from `from` (null when none is known). */
export function focusEntered<T>(
	kept: T | null,
	from: T | null,
	inCard: (el: T) => boolean
): T | null {
	return from !== null && !inCard(from) ? from : kept;
}

/** The element kept after focus moved within or out of the card, to `to` (null when nowhere). */
export function focusLeft<K, T>(
	kept: K | null,
	to: T | null,
	inCard: (el: T) => boolean
): K | null {
	return to !== null && inCard(to) ? kept : null;
}

/** The element to give focus back to on a dismissal, or null to leave focus alone. */
export function focusBack<T>(kept: T | null, byKeyboard: boolean, focusInCard: boolean): T | null {
	return byKeyboard && focusInCard ? kept : null;
}

/** Whether a click on the close button came from the keyboard: a keyboard click counts no presses. */
export function keyboardClick(detail: number): boolean {
	return detail === 0;
}

/** Whether a key dismisses the card. */
export function dismissKey(key: string): boolean {
	return key === 'Escape';
}
