/**
 * Toast store — module-singleton rune store mirroring the shape of
 * `download/store.svelte.ts`. Holds the ephemeral notifications
 * surfaced bottom-right of the window. Auto-dismiss timers are
 * owned by the store itself so call sites (play page, settings,
 * future Syncplay entry) don't have to manage their own timeouts.
 *
 * Stack policy: at most TOAST_MAX_STACK rows on-screen. Spam-clicks
 * trim the oldest entries so the corner doesn't fill up with stale
 * "Sent to mpv." rows piling on a single retry burst.
 */

export type ToastKind = 'success' | 'info' | 'warning' | 'error';

export interface ToastItem {
	id: string;
	kind: ToastKind;
	message: string;
	/** Auto-dismiss after this many ms. `null` pins the toast — only
	 *  user dismiss removes it. Useful for action-required toasts the
	 *  user shouldn't miss. */
	duration: number | null;
	actionLabel: string | null;
	onAction: (() => void) | null;
}

export interface PushArgs {
	kind: ToastKind;
	message: string;
	/** Defaults to 4000ms when omitted. `null` to pin. */
	duration?: number | null;
	actionLabel?: string;
	onAction?: () => void;
}

export const TOAST_MAX_STACK = 3;

const DEFAULT_DURATION_MS = 4000;

class ToastStore {
	items: ToastItem[] = $state([]);

	push(args: PushArgs): string {
		void args;
		throw new Error('test(red): toastStore.push() lands in the paired feat(green) commit');
	}

	dismiss(id: string): void {
		void id;
		throw new Error('test(red): toastStore.dismiss() lands in the paired feat(green) commit');
	}
}

export const toastStore = new ToastStore();

void DEFAULT_DURATION_MS;
