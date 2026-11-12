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

/** The Continue card standing for the history line `row` on Home: the
 *  cell drawn as that line, else the cell showing that row, which a
 *  click acts on when the cards' latest answer for it failed and they
 *  still draw the line the one before named. */
export function cardOf(root: ParentNode, row: string): HTMLElement | null {
	const id = CSS.escape(row);
	return (
		root.querySelector<HTMLElement>(`.resume-cell[data-card-row="${id}"] .resume-card`) ??
		root.querySelector<HTMLElement>(`.resume-cell[data-entry-id="${id}"] .resume-card`)
	);
}
