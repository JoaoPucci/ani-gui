/**
 * Which look the start veil takes (`$lib/play/start-veil`), kept per
 * machine in local storage beside the other display switches the
 * Diagnostics page owns:
 *
 *   - `still`: the episode's still, blurred and dimmed, under a
 *     centred film-strip loader. The default.
 *   - `card`: the same still under an editorial title card, the
 *     episode's numeral and titles, with the loader in its status line.
 *
 * Both are kept while the maintainer lives with them; one goes.
 */

import type { StorageLike } from '$lib/hero/prefs';

export type StartLook = 'still' | 'card';

export const START_LOOK_DEFAULT: StartLook = 'still';
export const START_LOOK_STORAGE_KEY = 'ani-gui:player-start-look';

export function parseStartLook(raw: string | null): StartLook {
	return raw === 'card' ? 'card' : START_LOOK_DEFAULT;
}

/** The kept look, or the default when there is no storage, storage
 *  refuses, or nothing is kept. */
export function loadStartLook(storage: StorageLike | null): StartLook {
	try {
		return parseStartLook(storage?.getItem(START_LOOK_STORAGE_KEY) ?? null);
	} catch {
		return START_LOOK_DEFAULT;
	}
}

/** Keeps the look; a storage that refuses keeps it for the session only. */
export function saveStartLook(storage: StorageLike | null, look: StartLook): void {
	try {
		storage?.setItem(START_LOOK_STORAGE_KEY, look);
	} catch {
		// Nothing to keep it in.
	}
}
