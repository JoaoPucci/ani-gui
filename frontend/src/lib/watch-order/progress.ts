/**
 * Progress per entry, from the watch history: an entry is watched once
 * a row reaches its last known episode, part-way otherwise. Rows
 * written before history recorded the Kitsu id cannot be placed and
 * are left out; several rows for one entry count the furthest.
 */
import type { HistoryEntry } from '$lib/api';
import { parseHistoryEpisode } from '$lib/play/episode-number';
import type { EntryProgress, Progress, WatchEntry } from './types';

/** The fields of a history row the progress reads. */
export type HistoryRow = Pick<HistoryEntry, 'kitsu_id' | 'ep_no'>;

/** Progress for each of `entries` that `rows` name. */
export function progressFromHistory(
	rows: readonly HistoryRow[],
	entries: readonly WatchEntry[]
): Progress {
	const furthest = new Map<string, number>();
	for (const row of rows) {
		const ep = parseHistoryEpisode(row.ep_no);
		if (row.kitsu_id === undefined || ep === null) continue;
		furthest.set(row.kitsu_id, Math.max(ep, furthest.get(row.kitsu_id) ?? 0));
	}
	const out: Record<string, EntryProgress> = {};
	for (const e of entries) {
		const ep = furthest.get(e.id);
		if (ep === undefined) continue;
		out[e.id] = e.episodes !== null && ep >= e.episodes ? 'watched' : ep;
	}
	return out;
}
