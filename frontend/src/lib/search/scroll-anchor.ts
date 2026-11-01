/**
 * The search page's DOM side of scroll restoration: where each result
 * card sits, and putting the anchored card back where it was. The rules
 * live in `snapshot.ts`.
 *
 * Positions come from the layout offsets, not `getBoundingClientRect`:
 * a card that has just arrived is still rising into place (its
 * entrance animation moves it with a transform), and its box would put
 * the scroll a few pixels off where it lands.
 */

import type { CardPosition, ScrollAnchor } from './snapshot';

/** The cards are marked with their show id. */
const CARD = '[data-card-id]';

/** How far below the top of the page `el` is laid out. */
function pageTop(el: HTMLElement): number {
	let top = 0;
	for (let n: HTMLElement | null = el; n; n = n.offsetParent as HTMLElement | null) {
		top += n.offsetTop;
	}
	return top;
}

/** Every result card under `root`, in order, in the coordinates of a
 *  viewport scrolled to `scrollY`. */
export function cardPositions(root: ParentNode | null, scrollY: number): CardPosition[] {
	if (!root) return [];
	return [...root.querySelectorAll<HTMLElement>(CARD)].map((el) => {
		const top = pageTop(el) - scrollY;
		return { id: el.dataset.cardId ?? '', top, bottom: top + el.offsetHeight };
	});
}

/** Scroll so the anchored card sits where it sat when the reader left;
 *  to the top when it is not on the page. */
export function scrollToAnchor(
	root: ParentNode | null,
	anchor: ScrollAnchor | null,
	win: Pick<Window, 'scrollTo'>
): void {
	const el =
		root && anchor
			? [...root.querySelectorAll<HTMLElement>(CARD)].find((c) => c.dataset.cardId === anchor.id)
			: undefined;
	const top = el && anchor ? Math.max(0, pageTop(el) - anchor.top) : 0;
	win.scrollTo({ top, left: 0, behavior: 'instant' });
}
