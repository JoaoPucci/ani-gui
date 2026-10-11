import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';
import { kitsuGroupSiblingIds } from './delete-group';
import { rowShow, rowShowIds, type RowShowDeps } from './row-show';

/**
 * Inputs the controller needs to execute a confirmed Continue
 * Watching delete. Kept narrow: the in-memory history snapshot,
 * the resolved Kitsu match map, and the per-id backend delete
 * IPC. The component owns the modal/busy state and is responsible
 * for re-applying `remainingHistory` to its $state after the
 * promise resolves.
 */
export interface ConfirmDeleteDeps extends RowShowDeps {
	history: HistoryEntry[];
	matches: Record<string, KitsuAnimeRef | null | undefined>;
	historyDelete: (id: string) => Promise<void>;
	/** The moment the removed rows are gone (watch-position-clear.ts): the
	 *  forgetting below, which can wait on Kitsu, takes only what was
	 *  written before it. */
	snapshotPositions?: () => number;
	/** Forgets where a removed show's episodes were left, once its
	 *  rows are gone, as of `since`. */
	forgetPositions?: (kitsuId: string, since?: number) => void;
	/** Forgets the positions written by sessions the removed rows'
	 *  Continue cards opened (watch-position-clear.ts) — under a guessed
	 *  match too, which a later load may have corrected — except those
	 *  under `keepShows`, the shows remaining rows are cards of — as of
	 *  `since`. */
	forgetRowPositions?: (rowIds: string[], keepShows: ReadonlySet<string>, since?: number) => void;
}

export interface ConfirmDeleteResult {
	/** Every id deleted from the backend — the clicked row plus
	 *  any Kitsu-group siblings the dedupe was hiding behind it. */
	removedIds: string[];
	/** History minus the removed group, suitable for an optimistic
	 *  local-state update. */
	remainingHistory: HistoryEntry[];
	/** Settles once the removed shows' kept positions are forgotten,
	 *  or found to stay. Never rejects. */
	forgetting: Promise<void>;
}

/**
 * Execute a confirmed Continue Watching delete for the clicked
 * entry. Two pieces of logic the home page used to inline:
 *
 *   1. Expand the clicked id to every history row in the same
 *      Kitsu group, so a dedupe-hidden sibling can't immediately
 *      become the new visible card (Codex P2 #3369138821).
 *   2. Serialize the backend `historyDelete` calls — each
 *      read-modify-writes the history file with an atomic rename.
 *      When this was written nothing made two of them take turns, so
 *      a parallel `Promise.all` could leave a sibling behind (Codex
 *      P2 #3369156513); the backend now holds the file across each
 *      delete, and the calls stay in sequence because nothing is
 *      gained by racing them.
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
	// Read before the deletes: removing a row takes its stamped mapping.
	const shows = await removedShows(groupIds, deps);
	for (const id of groupIds) {
		await deps.historyDelete(id);
	}
	// What is kept from here on is the user's since the removal.
	const snapshot = deps.snapshotPositions?.();
	const since: [] | [number] = snapshot === undefined ? [] : [snapshot];
	const removed = new Set(groupIds);
	const remainingHistory = deps.history.filter((e) => !removed.has(e.id));
	// Telling a remaining row's show can take a Kitsu read; the card's
	// removal does not wait on it.
	const forgetting = forgetLeftWithoutRows(shows, groupIds, remainingHistory, deps, since).catch(
		() => {}
	);
	return { removedIds: groupIds, remainingHistory, forgetting };
}

/** Forgets the positions of each removed row's show that no remaining
 *  row maps to, and those the removed rows' own sessions kept under a
 *  show no remaining row maps to. Positions belong to the show, and a
 *  surviving row of it is still a Continue card — a guessed card's
 *  play can land on, and record its watch under, another row of the
 *  show it played. No remaining row has a removed row's id: a delete
 *  takes every row of it. When a remaining row's show cannot be told,
 *  nothing is forgotten. Only what was kept before `since` goes. */
async function forgetLeftWithoutRows(
	shows: Set<string>,
	rowIds: string[],
	remaining: HistoryEntry[],
	deps: ConfirmDeleteDeps,
	since: [] | [number]
): Promise<void> {
	if (shows.size === 0 && !deps.forgetRowPositions) return;
	const still = await remainingShows(remaining, deps);
	if (still === null) return;
	for (const kitsuId of shows) if (!still.has(kitsuId)) deps.forgetPositions?.(kitsuId, ...since);
	deps.forgetRowPositions?.(rowIds, still, ...since);
}

/** The shows the remaining rows are cards of (row-show.ts), or null
 *  when a row's cannot be told: it may be the removed one. */
async function remainingShows(
	rows: HistoryEntry[],
	deps: ConfirmDeleteDeps
): Promise<Set<string> | null> {
	const told = await Promise.all(rows.map((row) => rowShow(row, deps)));
	const shows = new Set<string>();
	for (const show of told) {
		if (!show) return null;
		shows.add(show);
	}
	return shows;
}

/** The Kitsu ids the removed rows' positions can be kept under. */
async function removedShows(ids: string[], deps: ConfirmDeleteDeps): Promise<Set<string>> {
	const shows = new Set<string>();
	for (const id of ids) {
		const row = deps.history.find((e) => e.id === id) ?? { id, ep_no: '', title: '' };
		for (const kitsuId of await rowShowIds(row, deps)) shows.add(kitsuId);
	}
	return shows;
}
