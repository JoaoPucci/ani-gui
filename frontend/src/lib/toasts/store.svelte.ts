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
	/** What the toast's live region announces: its message, except where
	 *  an update is a silent one (`update(…, { silent: true })`), which
	 *  changes what is shown and leaves the announcement as it was. */
	announcement: string;
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
	/** Called once when the toast leaves the stack, by whatever route:
	 *  dismissed, timed out, or trimmed by a later push. */
	onClose?: () => void;
}

export const TOAST_MAX_STACK = 3;

const DEFAULT_DURATION_MS = 4000;

let nextId = 1;

class ToastStore {
	items: ToastItem[] = $state([]);
	/** Per toast id, what to call when it leaves the stack. */
	private closers = new Map<string, () => void>();

	push(args: PushArgs): string {
		const id = `toast-${nextId++}`;
		const duration = args.duration === undefined ? DEFAULT_DURATION_MS : args.duration;
		const row: ToastItem = {
			id,
			kind: args.kind,
			message: args.message,
			announcement: args.message,
			duration,
			actionLabel: args.actionLabel ?? null,
			onAction: args.onAction ?? null
		};
		// Trim oldest entries if a push would exceed the cap. Slice +
		// concat rather than splice so the assignment triggers
		// reactivity (matches download/store.svelte.ts's pattern).
		const trimmed = this.items.slice(-(TOAST_MAX_STACK - 1));
		const dropped = this.items.slice(0, this.items.length - trimmed.length);
		this.items = [...trimmed, row];
		if (args.onClose) this.closers.set(id, args.onClose);
		for (const gone of dropped) this.closed(gone.id);
		if (duration !== null) {
			// dismiss(id) is a no-op when the row has already been
			// trimmed by a later push — see the store test.
			setTimeout(() => this.dismiss(id), duration);
		}
		return id;
	}

	dismiss(id: string): void {
		const next = this.items.filter((i) => i.id !== id);
		if (next.length !== this.items.length) this.items = next;
		this.closed(id);
	}

	/** Change toast `id`'s message where it stands: the row is kept, so it
	 *  neither moves nor animates in again. A `silent` update changes what
	 *  is shown and not what is announced. A toast no longer on the stack
	 *  is left alone. */
	update(id: string, message: string, opts: { silent?: boolean } = {}): void {
		if (!this.items.some((i) => i.id === id && i.message !== message)) return;
		this.items = this.items.map((i) =>
			i.id === id ? { ...i, message, announcement: opts.silent ? i.announcement : message } : i
		);
	}

	private closed(id: string): void {
		const close = this.closers.get(id);
		this.closers.delete(id);
		close?.();
	}
}

export const toastStore = new ToastStore();
