/**
 * The window's one notification center. Creating it reads and writes
 * nothing: the layout calls `load()` only when the center's flag is
 * on, so with the flag off its storage is never touched.
 */
import { NoticeCenter, type NoticeStorage } from './center.svelte';
import type { Moment } from './arrival';
import type { Notice } from './model';

/** The window's local storage, or none where there is no window or
 *  the browser refuses it. */
export function browserStorage(
	win: { readonly localStorage: NoticeStorage } | undefined
): NoticeStorage | null {
	try {
		return win?.localStorage ?? null;
	} catch {
		return null;
	}
}

let toast: ((n: Notice) => void) | null = null;
let moment: (() => Moment) | null = null;
let observer: ((before: readonly Notice[], after: readonly Notice[]) => void) | null = null;

/** Who is told of each change to the list: the layout points this at
 *  what it remembers of published notices. */
export function setNoticeObserver(
	fn: ((before: readonly Notice[], after: readonly Notice[]) => void) | null
): void {
	observer = fn;
}

/** How the center reads what the window is doing at each arrival: the
 *  layout points this at the document. A video the player removes on
 *  leaving fires its pause after it has left the page, where no
 *  listener hears it, so the moment is read fresh, not remembered. */
export function setMomentReader(fn: (() => Moment) | null): void {
	moment = fn;
}

/** Where the center's toasts go: the layout points this at the app's
 *  toast stack. */
export function setNoticeToast(fn: (n: Notice) => void): void {
	toast = fn;
}

export const noticeCenter: NoticeCenter = new NoticeCenter({
	storage: browserStorage(globalThis as unknown as { readonly localStorage: NoticeStorage }),
	now: () => Date.now(),
	toast: (n) => toast?.(n),
	moment: (): Moment => moment?.() ?? noticeCenter.moment,
	observe: (before, after) => observer?.(before, after)
});
