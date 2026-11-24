/**
 * Holding the reader's place while the show page's held watch-order
 * room goes above the viewport (`./arrival` has the rules): where the
 * page was before the change, and the scroll that puts it back once the
 * change is laid out.
 */
import { scrollBack, type Gone } from './arrival';

/** The window's scroll position and the call that sets it. */
export interface ScrollWindow {
	readonly scrollY: number;
	scrollTo(x: number, y: number): void;
}

/**
 * The window's scroll as `holdScroll` reads it: the position read live
 * (a copied value would be the position at the wrong moment), the call
 * wrapped, not handed over as a method.
 */
export function windowScroll(): ScrollWindow {
	return {
		get scrollY() {
			return window.scrollY;
		},
		scrollTo: (x, y) => window.scrollTo(x, y)
	};
}

/**
 * Keep what the reader is reading in place as the held room `gone`
 * goes. Made before the change, it notes where the page is; called once
 * the change is laid out, with the drawn block's height, it scrolls to
 * that position moved by what the change gave back or took. The target
 * is absolute: removing the room can shorten the page, and the browser
 * clamps the scroll position before this runs.
 */
export function holdScroll(win: ScrollWindow, gone: Gone): (drawnHeight: number) => void {
	const before = win.scrollY;
	return (drawnHeight) => {
		const dy = scrollBack(gone, drawnHeight);
		if (dy !== 0) win.scrollTo(0, before + dy);
	};
}
