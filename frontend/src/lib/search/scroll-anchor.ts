/**
 * The search page's DOM side of scroll restoration: where each result
 * card sits, and putting the anchored card back where it was. The rules
 * live in `snapshot.ts`.
 */

import type { CardPosition, ScrollAnchor } from './snapshot';

/** Declared ahead of its rules. */
export function cardPositions(root: ParentNode | null, scrollY: number): CardPosition[] {
	void root;
	void scrollY;
	return [];
}

/** Declared ahead of its rules. */
export function scrollToAnchor(
	root: ParentNode | null,
	anchor: ScrollAnchor | null,
	win: Pick<Window, 'scrollTo'>
): void {
	void root;
	void anchor;
	void win;
}
