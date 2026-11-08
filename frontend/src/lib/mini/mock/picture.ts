/**
 * The picture's pointer rules. A click on the picture while a menu or
 * the on-top explainer is open only closes it; otherwise it plays or
 * pauses after PICTURE_CLICK_DELAY_MS, so a double-click (back to the
 * app) does not play and pause on the way.
 *
 * `seekFraction` is where along the timeline a pointer is, from 0 to
 * 1, read from the inline end in a right-to-left layout.
 */

export const PICTURE_CLICK_DELAY_MS = 220;

export type PictureClick = 'closeMenus' | 'toggleLater';

export function decidePictureClick(menuOpen: boolean): PictureClick {
	return menuOpen ? 'closeMenus' : 'toggleLater';
}

export function seekFraction(clientX: number, left: number, width: number, rtl: boolean): number {
	if (width <= 0) return 0;
	const f = Math.max(0, Math.min(1, (clientX - left) / width));
	return rtl ? 1 - f : f;
}
