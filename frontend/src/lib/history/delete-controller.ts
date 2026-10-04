import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { kitsuGroupSiblingIds } from './delete-group';

/**
 * Inputs the controller needs to execute a confirmed Continue
 * Watching delete. Kept narrow: the in-memory history snapshot,
 * the resolved Kitsu match map, and the per-id backend delete
 * IPC. The component owns the modal/busy state and is responsible
 * for re-applying `remainingHistory` to its $state after the
 * promise resolves.
 */
export interface ConfirmDeleteDeps {
	history: HistoryEntry[];
	matches: Record<string, KitsuAnimeRef | null | undefined>;
	historyDelete: (id: string) => Promise<void>;
	/** Forgets where a removed show's episodes were left, once its
	 *  rows are gone. */
	forgetPositions?: (kitsuId: string) => void;
	/** The Kitsu id a play stamped for a row's show id — for a row
	 *  whose match never resolved. */
	kitsuIdOf?: (showId: string) => Promise<string | null>;
}

export interface ConfirmDeleteResult {
	/** Every id deleted from the backend — the clicked row plus
	 *  any Kitsu-group siblings the dedupe was hiding behind it. */
	removedIds: string[];
	/** History minus the removed group, suitable for an optimistic
	 *  local-state update. */
	remainingHistory: HistoryEntry[];
}

/**
 * Execute a confirmed Continue Watching delete for the clicked
 * entry. Two pieces of logic the home page used to inline:
 *
 *   1. Expand the clicked id to every history row in the same
 *      Kitsu group, so a dedupe-hidden sibling can't immediately
 *      become the new visible card (Codex P2 #3369138821).
 *   2. Serialize the backend `historyDelete` calls — they
 *      read-modify-write the history file with an atomic rename and no
 *      shared lock, so a parallel `Promise.all` can leave a
 *      sibling behind (Codex P2 #3369156513).
 *
 * Returning the filtered history (rather than mutating) keeps
 * the function pure and lets the test assert ordering + filter
 * without rigging up Svelte's `$state`.
 */
export async function executeKitsuGroupDelete(
	clickedId: string,
	deps: ConfirmDeleteDeps
): Promise<ConfirmDeleteResult> {
	const groupIds = kitsuGroupSiblingIds(clickedId, deps.history, deps.matches);
	for (const id of groupIds) {
		await deps.historyDelete(id);
	}
	const removed = new Set(groupIds);
	const remainingHistory = deps.history.filter((e) => !removed.has(e.id));
	await forgetShowsLeftWithoutRows(groupIds, remainingHistory, deps);
	return { removedIds: groupIds, remainingHistory };
}

/** Forgets the positions of each removed row's show that no remaining
 *  row maps to. Positions belong to the show, and a surviving row of
 *  it is still a Continue card. When a remaining row's show cannot be
 *  told, nothing is forgotten. */
async function forgetShowsLeftWithoutRows(
	removedIds: string[],
	remaining: HistoryEntry[],
	deps: ConfirmDeleteDeps
): Promise<void> {
	const shows = await removedShows(removedIds, deps);
	if (shows.size === 0) return;
	const still = await remainingShows(remaining, deps);
	if (still === null) return;
	for (const kitsuId of shows) if (!still.has(kitsuId)) deps.forgetPositions?.(kitsuId);
}

/** The shows the remaining rows map to — each row's resolved match,
 *  else its stamped mapping — or null when a row's cannot be told. */
async function remainingShows(
	rows: HistoryEntry[],
	deps: ConfirmDeleteDeps
): Promise<Set<string> | null> {
	const shows = new Set<string>();
	for (const row of rows) {
		const resolved = deps.matches[row.id]?.id;
		if (resolved) {
			shows.add(resolved);
			continue;
		}
		// No way to ask, a failed lookup and no stamped mapping all
		// leave the row's show untold: it may be the removed one.
		const mapped = await deps.kitsuIdOf?.(row.id).catch(() => null);
		if (!mapped) return null;
		shows.add(mapped);
	}
	return shows;
}

/** The Kitsu ids of the removed rows' shows: each row's resolved
 *  match, or, for a row whose match never resolved, the mapping its
 *  plays stamped. A mapping that cannot be read names nothing. */
async function removedShows(ids: string[], deps: ConfirmDeleteDeps): Promise<Set<string>> {
	const shows = new Set<string>();
	for (const id of ids) {
		const kitsuId = deps.matches[id]?.id ?? (await deps.kitsuIdOf?.(id).catch(() => null)) ?? null;
		if (kitsuId) shows.add(kitsuId);
	}
	return shows;
}
