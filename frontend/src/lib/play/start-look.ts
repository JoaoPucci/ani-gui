/**
 * Which look the start veil takes, kept per machine in local storage
 * beside the other display switches the Diagnostics page owns.
 *
 * The signatures only, so far: the behaviour comes with its tests.
 */

import type { StorageLike } from '$lib/hero/prefs';

export type StartLook = 'still' | 'card';

export const START_LOOK_DEFAULT: StartLook = 'still';
export const START_LOOK_STORAGE_KEY = 'ani-gui:player-start-look';

export function parseStartLook(raw: string | null): StartLook {
	void raw;
	return 'card';
}

export function loadStartLook(storage: StorageLike | null): StartLook {
	void storage;
	return 'card';
}

export function saveStartLook(storage: StorageLike | null, look: StartLook): void {
	void storage;
	void look;
}
