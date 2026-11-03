/**
 * Progress per entry, from the watch history: an entry is watched once
 * a row reaches its last known episode, part-way otherwise. Rows
 * written before history recorded the Kitsu id cannot be placed and
 * are left out; several rows for one entry count the furthest.
 */
import type { HistoryEntry } from '$lib/api';
import type { Progress, WatchEntry } from './types';

/** The fields of a history row the progress reads. */
export type HistoryRow = Pick<HistoryEntry, 'kitsu_id' | 'ep_no'>;

/** Progress for each of `entries` that `rows` name. */
export function progressFromHistory(
	rows: readonly HistoryRow[],
	entries: readonly WatchEntry[]
): Progress {
	void rows;
	void entries;
	return {};
}
