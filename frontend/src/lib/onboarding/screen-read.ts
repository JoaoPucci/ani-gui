/**
 * What the spotlight reads off the screen for the tip on it: the
 * window, the top bar's bottom edge, the place, and the window's own
 * buttons, in viewport pixels. Rendering glue.
 */
import type { Rect } from './place';
import { sameRect } from './spotlight';

/** The window's own buttons: minimize, maximize, close. */
export const WINDOW_BUTTONS = '.winctl-btn';

export interface ScreenRead {
	view: { w: number; h: number; top: number };
	rect: Rect;
	buttons: Rect[];
}

export const toRect = (r: DOMRect): Rect => ({ x: r.left, y: r.top, w: r.width, h: r.height });

/** Read the screen around `place`. */
export function readScreen(doc: Document, place: HTMLElement): ScreenRead {
	const win = doc.defaultView;
	const top = doc.querySelector('.topbar')?.getBoundingClientRect().bottom ?? 0;
	return {
		view: { w: win?.innerWidth ?? 0, h: win?.innerHeight ?? 0, top: Math.round(top) },
		rect: toRect(place.getBoundingClientRect()),
		buttons: [...doc.querySelectorAll<HTMLElement>(WINDOW_BUTTONS)].map((b) =>
			toRect(b.getBoundingClientRect())
		)
	};
}

/** Whether two reads of the window agree. */
export const sameView = (a: ScreenRead['view'], b: ScreenRead['view']): boolean =>
	a.w === b.w && a.h === b.h && a.top === b.top;

/** Whether two lists of rects agree. */
export const sameRects = (a: Rect[], b: Rect[]): boolean =>
	a.length === b.length && a.every((r, i) => sameRect(r, b[i]));

/** The show's colour at `el` (its `--accent`), or `fallback`. */
export function accentAt(doc: Document, el: HTMLElement, fallback: string): string {
	return doc.defaultView?.getComputedStyle(el).getPropertyValue('--accent').trim() || fallback;
}

/** Call `changed` whenever `place` changes size, or anything on the
 *  page mounts or unmounts (which may move it or take it away), until
 *  the returned function is called. */
export function observePlace(doc: Document, place: HTMLElement, changed: () => void): () => void {
	const resize = new ResizeObserver(changed);
	resize.observe(place);
	const mounts = new MutationObserver(changed);
	mounts.observe(doc.body, { childList: true, subtree: true });
	return () => {
		resize.disconnect();
		mounts.disconnect();
	};
}
