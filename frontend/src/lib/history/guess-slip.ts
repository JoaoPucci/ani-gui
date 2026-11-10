/**
 * What the player's slip for a guessed session shows, the rows whose
 * slip the viewer hid, and the rows the viewer picked a show for on the
 * player — split from $lib/history/in-place-link.svelte.ts for the
 * per-file complexity bar.
 *
 * Hide lasts until the app restarts, and so does a pick's way to change
 * the show again: both are module-level sets, which live as long as the
 * renderer. A switch to a pick tells the session it opens once
 * (`noteSwitch`, `takeSwitch`).
 */

/** What the slip shows: nothing, its held room while the offer is
 *  decided, the question, the note after a pick the viewer kept
 *  watching past, or the note of the session a switch opened. */
export type SlipState = 'none' | 'holding' | 'guess' | 'linked' | 'switched';

export interface SlipInputs {
	/** The card's history row; null for a session no card opened. */
	row: string | null;
	/** Whether the viewer hid the row's slip. */
	aside: boolean;
	/** Whether another show was picked for the card from here, while
	 *  what plays is still the one before. */
	linked: boolean;
	/** Whether the session was opened by a switch to a pick, and has not
	 *  been told otherwise since. */
	switched: boolean;
	/** Whether the offer is decided. */
	settled: boolean;
	/** Whether there is an offer: the ⋯ entry's way to the picker. */
	offered: boolean;
	/** Whether the offer still asks: a guess, not a pick kept only to
	 *  change the show again. */
	asking: boolean;
	/** Whether the session can ask at all: a guess opened it. One that
	 *  cannot only changes a named show again, from the ⋯ entry, and
	 *  holds no room for a question it will never ask. */
	asks?: boolean;
}

export function slipState(s: SlipInputs): SlipState {
	if (s.row === null || s.aside) return 'none';
	if (s.linked) return 'linked';
	if (s.switched) return 'switched';
	if (s.asks === false) return 'none';
	if (!s.settled) return 'holding';
	return s.offered && s.asking ? 'guess' : 'none';
}

const hiddenRows = new Set<string>();
const pickedRows = new Set<string>();
let switchNote: { row: string; title: string } | null = null;

/** Hide the slip of `row` until the app restarts. */
export function hideRow(row: string): void {
	hiddenRows.add(row);
}

/** Whether the viewer hid the slip of `row` since the app started. */
export function isRowHidden(row: string | null): boolean {
	return row !== null && hiddenRows.has(row);
}

/** Note that the viewer picked a show for `row` on the player: its
 *  sessions keep the way to change it again until the app restarts. */
export function notePick(row: string): void {
	pickedRows.add(row);
}

/** Whether the viewer picked a show for `row` on the player since the
 *  app started. */
export function pickedRow(row: string | null): boolean {
	return row !== null && pickedRows.has(row);
}

/** Tell the next session of `row`, the one a switch opens, that it
 *  plays `title` now. */
export function noteSwitch(row: string, title: string): void {
	switchNote = { row, title };
}

/** What the session of `row` is to say it switched to, once; null when
 *  nothing. Taking it clears it either way, so a note meant for another
 *  row never reaches a later one. */
export function takeSwitch(row: string | null): string | null {
	const note = switchNote;
	switchNote = null;
	return note !== null && note.row === row ? note.title : null;
}

/** Test seam: forget every hidden row and pick, as a restart does. */
export function __resetHiddenRowsForTests(): void {
	hiddenRows.clear();
	pickedRows.clear();
	switchNote = null;
}
