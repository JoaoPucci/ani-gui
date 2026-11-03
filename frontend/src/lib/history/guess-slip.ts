/**
 * What the player's slip for a guessed session shows, and the rows
 * whose slip the viewer hid — split from
 * $lib/history/in-place-link.svelte.ts for the per-file complexity bar.
 *
 * Hide lasts until the app restarts: the hidden rows are a module-level
 * set, which lives as long as the renderer.
 */

/** What the slip shows: nothing, its held room while the offer is
 *  decided, the guess, or the note after a link elsewhere. */
export type SlipState = 'none' | 'holding' | 'guess' | 'linked';

export interface SlipInputs {
	/** The card's history row; null for a session no guess opened. */
	row: string | null;
	/** Whether the viewer hid the row's slip. */
	aside: boolean;
	/** Whether the card was linked to another show from here. */
	linked: boolean;
	/** Whether the offer is decided. */
	settled: boolean;
	/** Whether the row is still offered as a guess. */
	offered: boolean;
}

export function slipState(s: SlipInputs): SlipState {
	if (s.row === null || s.aside) return 'none';
	if (s.linked) return 'linked';
	if (!s.settled) return 'holding';
	return s.offered ? 'guess' : 'none';
}

const hiddenRows = new Set<string>();

/** Hide the slip of `row` until the app restarts. */
export function hideRow(row: string): void {
	hiddenRows.add(row);
}

/** Whether the viewer hid the slip of `row` since the app started. */
export function isRowHidden(row: string | null): boolean {
	return row !== null && hiddenRows.has(row);
}

/** Test seam: forget every hidden row, as a restart does. */
export function __resetHiddenRowsForTests(): void {
	hiddenRows.clear();
}
