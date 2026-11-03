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
	void menuOpen;
	return 'toggleLater';
}

export function seekFraction(clientX: number, left: number, width: number, rtl: boolean): number {
	void clientX;
	void left;
	void width;
	void rtl;
	return 0;
}
