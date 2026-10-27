// How the list editor treats Rewatching across the connected trackers.
// Pure + unit-tested; the editor and the save fan-out read them.
//
// AniList keeps "you already finished this show" in a rewatch status of its
// own (REPEATING). MyAnimeList has none: a row it reads as rewatching is
// listed as Watching, and "finished" lives only in Completed, so a rewatch
// written to it would erase the record that the user finished the show.

import type { EntryView, ListStatus, Provider } from './types';

/** Whether a tracker has a rewatch status of its own. */
export function holdsRewatches(provider: Provider): boolean {
	return provider !== 'mal';
}

/**
 * The status options the editor offers once the connected trackers are
 * known: Rewatching only while a connected tracker can hold one, or where
 * the entry already is one, so the editor reflects what a tracker holds.
 */
export function statusOptionsForTrackers(
	options: ListStatus[],
	connected: Provider[],
	current: ListStatus | null
): ListStatus[] {
	if (current === 'rewatching' || connected.length === 0 || connected.some(holdsRewatches)) {
		return options;
	}
	return options.filter((s) => s !== 'rewatching');
}

/** Whether the editor says, beside the picked status, that a connected
 *  tracker keeps its entry as it is: Rewatching picked with a tracker that
 *  has no rewatch status connected. */
export function rewatchNoteShown(connected: Provider[], status: ListStatus): boolean {
	return status === 'rewatching' && connected.some((p) => !holdsRewatches(p));
}

/**
 * The edit to send a tracker, given its own `current` row: as built, except
 *   - a Completed row whose status the edit leaves alone is never written
 *     below its count (a rewatch's count, beside it, when the total is
 *     unknown and cannot snap Completed to it);
 *   - a tracker without a rewatch status is sent no Rewatching: its row keeps
 *     its status — nothing at all is written to a Completed row, and no row
 *     is created — while any other row still takes the progress.
 * `null` when nothing is to be written: the tracker is kept as it is.
 */
export function editForTracker<E extends { status?: ListStatus; progress: number }>(
	provider: Provider,
	edit: E,
	current: EntryView | null
): Omit<E, 'status'> | E | null {
	if (current?.status === 'completed' && edit.status === undefined) {
		if (edit.progress < current.progress) return null;
	}
	if (holdsRewatches(provider) || edit.status !== 'rewatching') return edit;
	if (current === null || current.status === 'completed') return null;
	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	const { status, ...rest } = edit;
	return rest;
}
