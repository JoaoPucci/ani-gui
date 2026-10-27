// How the list editor treats Rewatching across the connected trackers.
// Pure + unit-tested; the editor and the save fan-out read them.

import type { ListStatus, Provider } from './types';

/**
 * The status options the editor offers once the connected trackers are
 * known.
 */
export function statusOptionsForTrackers(
	options: ListStatus[],
	connected: Provider[],
	current: ListStatus | null
): ListStatus[] {
	void connected;
	void current;
	return options;
}

/** Whether the editor says, beside the picked status, that a connected
 *  tracker keeps its entry as it is. */
export function rewatchNoteShown(connected: Provider[], status: ListStatus): boolean {
	void connected;
	void status;
	return false;
}
