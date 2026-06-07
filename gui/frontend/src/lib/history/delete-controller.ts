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
 *      read-modify-write `ani-hsts` with an atomic rename and no
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
	return {
		removedIds: groupIds,
		remainingHistory: deps.history.filter((e) => !removed.has(e.id))
	};
}
