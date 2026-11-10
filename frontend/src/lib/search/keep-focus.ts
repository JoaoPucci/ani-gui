/**
 * Hands focus on when a control leaves while holding it. The search
 * page's buttons unmount with the state they belong to — the paging
 * folio's Try again as its page loads and Load more at the search's
 * end, the discovery notice's Try again as the rows load again — and
 * focus that sat on one would otherwise fall to the page body.
 */

/** A control that can report focus, and a destination that takes it. */
export interface FocusSource {
	addEventListener(type: 'focus' | 'blur', listener: () => void): void;
	removeEventListener(type: 'focus' | 'blur', listener: () => void): void;
	readonly isConnected: boolean;
}

/** Svelte action: when `node` is torn down while it held focus,
 *  focus `target()` without scrolling to it. */
export function keepFocus(
	node: FocusSource,
	target: () => { focus(options?: FocusOptions): void } | null
): { destroy(): void } {
	let held = false;
	const onFocus = () => {
		held = true;
	};
	// A blur fired by the control's own removal is not the reader
	// moving on: it is checked once the removal has happened.
	const onBlur = () => {
		queueMicrotask(() => {
			if (node.isConnected) held = false;
		});
	};
	node.addEventListener('focus', onFocus);
	node.addEventListener('blur', onBlur);
	return {
		destroy() {
			node.removeEventListener('focus', onFocus);
			node.removeEventListener('blur', onBlur);
			if (held) target()?.focus({ preventScroll: true });
		}
	};
}
