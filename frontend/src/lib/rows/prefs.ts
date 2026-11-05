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

function styleOr(v: unknown, fallback: RowStyle): RowStyle {
	return ROW_STYLES.find((s) => s === v) ?? fallback;
}

/** Read stored preferences, falling back to the default per field. */
export function parseRowPrefs(raw: string | null): RowPrefs {
	let v: unknown;
	try {
		v = JSON.parse(raw ?? 'null');
	} catch {
		v = null;
	}
	if (typeof v !== 'object' || v === null || Array.isArray(v)) return ROW_PREFS_DEFAULT;
	const o = v as Record<string, unknown>;
	return {
		style: styleOr(o.style, ROW_PREFS_DEFAULT.style),
		controls: o.controls === true
	};
}

/** The stored preferences, or the default when there is no storage,
 *  storage refuses, or nothing is stored. */
export function loadRowPrefs(storage: StorageLike | null): RowPrefs {
	let stored: string | null;
	try {
		stored = storage?.getItem(ROW_PREFS_STORAGE_KEY) ?? null;
	} catch {
		stored = null;
	}
	return parseRowPrefs(stored);
}

/** Keep `prefs` for the next load. A refusal is not an error: the
 *  caller already holds the preferences for this session. */
export function saveRowPrefs(storage: StorageLike | null, prefs: RowPrefs): void {
	try {
		storage?.setItem(ROW_PREFS_STORAGE_KEY, JSON.stringify(prefs));
	} catch {
		// Not persisted: this session still uses it.
	}
}

/** How a row action is drawn in a row style:
 *   - `icon`:  at rest only its icon, in a 32px circle; on hover and
 *              keyboard focus it widens to the labelled pill. Beside
 *              the title the row has no surface of its own, so the
 *              header line stays quiet until an action is reached for.
 *   - `label`: the labelled pill, its words always shown. A band
 *              already marks the row out, and its actions sit at the
 *              band's end with room to spare. */
export type RowActionShape = 'icon' | 'label';

/** The shape a row's header actions take in `style`. */
export function actionShape(style: RowStyle): RowActionShape {
	return style === 'inline' ? 'icon' : 'label';
}
