/**
 * Where keyboard focus goes back to when a modal closes. A dialog
 * takes focus when it opens; when it closes, focus should land on the
 * control that opened it — the "Clear history" action, say — rather
 * than falling to the page, which sends a keyboard user back to the
 * top. If that control has gone in the meantime (clearing removed its
 * row), there is nowhere to return to and focus is left alone.
 */

/** The part of an element this needs: whether it is still in the
 *  document, and a way to focus it. */
export interface Focusable {
	readonly isConnected: boolean;
	focus(): void;
}

export interface FocusReturn {
	/** Note what holds focus now, before the dialog takes it. */
	save(active: Focusable | null): void;
	/** Give focus back to what was noted, if it is still there. */
	restore(): void;
}

export function createFocusReturn(): FocusReturn {
	throw new Error('not implemented');
}
