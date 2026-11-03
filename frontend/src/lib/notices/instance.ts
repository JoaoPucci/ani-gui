/**
 * The window's one notification center. Creating it reads and writes
 * nothing: the layout calls `load()` only when the center's flag is
 * on, so with the flag off its storage is never touched.
 */
import { NoticeCenter, type NoticeStorage } from './center.svelte';
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

/** Where the center's toasts go: the layout points this at the app's
 *  toast stack. */
export function setNoticeToast(fn: (n: Notice) => void): void {
	toast = fn;
}

export const noticeCenter = new NoticeCenter({
	storage: browserStorage(globalThis as unknown as { readonly localStorage: NoticeStorage }),
	now: () => Date.now(),
	toast: (n) => toast?.(n)
});
