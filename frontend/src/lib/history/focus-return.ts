/**
 * Where a dialog gives focus back when it closes — split from
 * LinkShowDialog.svelte so the rule is tested on its own.
 *
 * The element that had focus when the dialog opened gets it back. Focus
 * left on the page itself (`document.body`, as a closed overlay leaves
 * it) is no place to go back to, and neither is an element no longer in
 * the document: the surface's own `fallback` names one instead, as Home
 * names the card a failed resume's picker was opened for.
 */

/** What the dialog remembers as its opener, from what has focus now. */
export function openerOf(active: Element | null, body: Element | null): HTMLElement | null {
	return active instanceof HTMLElement && active !== body ? active : null;
}

/** Where focus goes on close. */
export function focusBack(
	opener: HTMLElement | null,
	fallback?: () => HTMLElement | null
): HTMLElement | null {
	return opener?.isConnected ? opener : (fallback?.() ?? null);
}
