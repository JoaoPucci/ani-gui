/**
 * Where Tab goes inside a modal. A modal keeps the keyboard to itself:
 * Tab moves to the next control in it and Shift+Tab to the one before,
 * and at either end they wrap round rather than leaving for the page
 * behind. `$lib/components/modal-layer` asks this on every Tab.
 */

/**
 * The control Tab (or, `backwards`, Shift+Tab) moves to from `current`,
 * among `items` in their Tab order, wrapping at both ends. Focus that is
 * on none of them — on the modal's own surface, say — enters at the
 * first, or at the last going backwards. Null when nothing in the modal
 * takes focus.
 */
export function nextInCycle<T>(
	items: readonly T[],
	current: T | null,
	backwards: boolean
): T | null {
	if (items.length === 0) return null;
	const at = current === null ? -1 : items.indexOf(current);
	if (at === -1) return backwards ? items[items.length - 1] : items[0];
	const step = backwards ? items.length - 1 : 1;
	return items[(at + step) % items.length];
}

const CANDIDATES = [
	'a[href]',
	'button',
	'input:not([type="hidden"])',
	'select',
	'textarea',
	'[tabindex]'
].join(', ');

/**
 * The controls in `root` that Tab stops at, in document order: what can
 * take focus and is not taken out of the order (a negative tabindex),
 * disabled, hidden or inert.
 */
export function tabbablesIn(root: Element): HTMLElement[] {
	return Array.from(root.querySelectorAll<HTMLElement>(CANDIDATES)).filter(takesTab);
}

function takesTab(el: HTMLElement): boolean {
	if (el.tabIndex < 0 || (el as HTMLButtonElement).disabled === true) return false;
	return el.closest('[hidden], [inert]') === null;
}
