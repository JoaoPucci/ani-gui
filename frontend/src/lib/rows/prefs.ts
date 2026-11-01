/**
 * Home row preferences, kept per machine in local storage beside the
 * hero's (`$lib/hero/prefs`), in the same shape:
 *
 *   - `style` — how a home row presents its header actions:
 *       `inline`: the actions continue the row's title line, in its
 *                 type, the same gap between every item on it
 *       `band`:   every row sits on a faint surface that marks out its
 *                 extent; the actions sit at that surface's end
 *   - `controls` — whether the on-page switch that changes the style is
 *     shown; the diagnostics page owns it
 *
 * `inline` is the default: it ties an action to its row by position,
 * not by decoration, and adds nothing to the page around it.
 */

import type { StorageLike } from '$lib/hero/prefs';

export type RowStyle = 'inline' | 'band';

export interface RowPrefs {
	style: RowStyle;
	controls: boolean;
}

export const ROW_PREFS_DEFAULT: RowPrefs = { style: 'inline', controls: false };
export const ROW_PREFS_STORAGE_KEY = 'ani-gui:row-style';
export const ROW_STYLES: readonly RowStyle[] = ['inline', 'band'];

/** Read stored preferences, falling back to the default per field. */
export function parseRowPrefs(raw: string | null): RowPrefs {
	void raw;
	throw new Error('not implemented');
}

/** The stored preferences, or the default when there is no storage,
 *  storage refuses, or nothing is stored. */
export function loadRowPrefs(storage: StorageLike | null): RowPrefs {
	void storage;
	throw new Error('not implemented');
}

/** Keep `prefs` for the next load. A refusal is not an error: the
 *  caller already holds the preferences for this session. */
export function saveRowPrefs(storage: StorageLike | null, prefs: RowPrefs): void {
	void storage;
	void prefs;
	throw new Error('not implemented');
}
