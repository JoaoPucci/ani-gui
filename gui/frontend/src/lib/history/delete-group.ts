import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

/**
 * Given a Continue Watching entry that the user wants to delete,
 * return every history id that resolves to the same Kitsu id —
 * i.e. every row that the home page's `dedupeHistoryByKitsuId` was
 * hiding behind the same visible card. Without this, deleting a
 * card leaves its catalog-drift siblings in storage, and the next
 * dedupe pass surfaces one of them so the title looks un-deleted.
 *
 * Falls back to `[entryId]` when the entry itself has no resolved
 * match — without a Kitsu id there is no group to expand to.
 */
export function kitsuGroupSiblingIds(
	entryId: string,
	history: HistoryEntry[],
	matches: Record<string, KitsuAnimeRef | null | undefined>
): string[] {
	const myMatch = matches[entryId];
	if (!myMatch) return [entryId];
	const kitsuId = myMatch.id;
	return history.filter((e) => matches[e.id]?.id === kitsuId).map((e) => e.id);
}
