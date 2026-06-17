// Fan-out for the detail-page list editor: push an explicit edit (or a
// removal) to every connected tracker. Renderer-driven — the backend is
// stateless, so the renderer holds the per-provider bearers and
// orchestrates the calls (mirrors push-watched.ts).

import type { EntryView, ListEntry, ListStatus, Provider } from './types';
import { getEntry, removeEntry, setEntry } from './entry-api';
import { buildListEdit } from './list-entry-view';
import { accountStore } from './store.svelte';
import { freshBearerFor } from './fresh-bearer';
import { invalidateWatchLater } from './watch-later-refresh';

/**
 * A deliberate edit from the detail-page editor. `status` is the value
 * shown in the editor (the seed, or what the user picked); `seededStatus`
 * is what the editor opened on — the fan-out compares the two per tracker
 * to decide whether to write status, so it only converges a divergent
 * status when the user actually changed it.
 */
export interface EditorSave {
	status: ListStatus;
	seededStatus: ListStatus;
	progress: number;
}

export interface SetEntryDeps {
	/** Providers currently connected (from the account store). */
	connected: Provider[];
	/** Resolve a provider's bearer, or null if unavailable. May be async
	 *  so the caller can refresh a near-expiry token first. */
	bearerFor: (provider: Provider) => string | null | Promise<string | null>;
	/** Read one provider's current entry (null when the show isn't on it). */
	getEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<EntryView | null>;
	/** POST the per-provider body to one provider. */
	setEntry: (
		provider: Provider,
		bearer: string,
		body: { kitsu_id: string; status?: string; progress?: number }
	) => Promise<ListEntry | null>;
}

export interface RemoveEntryDeps {
	connected: Provider[];
	bearerFor: (provider: Provider) => string | null | Promise<string | null>;
	getEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<EntryView | null>;
	removeEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<void>;
}

/**
 * Write the editor's save to every connected tracker, deciding per
 * provider off that provider's live entry: send `status` only where the
 * row is missing (so it's created with the editor's status) or where the
 * user changed it; otherwise send progress alone so a tracker keeps its
 * own status. Best-effort — a missing bearer is skipped, and an unmappable
 * show (`setEntry` → null) or a thrown error counts as not-written without
 * blocking the others. Returns how many providers accepted the write.
 */
export async function setEntryAcrossTrackers(
	deps: SetEntryDeps,
	kitsuId: string,
	save: EditorSave
): Promise<number> {
	if (!kitsuId || deps.connected.length === 0) return 0;
	const results = await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return false;
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				const edit = buildListEdit({
					onList: current !== null,
					seededStatus: save.seededStatus,
					status: save.status,
					progress: save.progress
				});
				return (await deps.setEntry(provider, bearer, { kitsu_id: kitsuId, ...edit })) !== null;
			} catch {
				return false;
			}
		})
	);
	return results.filter(Boolean).length;
}

/**
 * Remove a show from every connected tracker that actually has it. Reads
 * each provider first: a provider without the row is skipped (its delete
 * would be a no-op and must not count toward success), so an already-absent
 * tracker can't mask a real provider's failed delete. Returns the count of
 * providers that had the entry and removed it.
 */
export async function removeEntryAcrossTrackers(
	deps: RemoveEntryDeps,
	kitsuId: string
): Promise<number> {
	if (!kitsuId || deps.connected.length === 0) return 0;
	const results = await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return false;
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				if (current === null) return false; // nothing to remove here
				await deps.removeEntry(provider, bearer, kitsuId);
				return true;
			} catch {
				return false;
			}
		})
	);
	return results.filter(Boolean).length;
}

/**
 * Live-store wiring: fan the editor's save out to every connected tracker
 * off the account store, refreshing a near-expiry bearer first, then
 * invalidate the Watch Later snapshot (a status change can cross the
 * planning boundary). Returns the success count for toast feedback.
 */
export async function syncSetEntry(kitsuId: string, save: EditorSave): Promise<number> {
	const connected = accountStore.connected;
	const n = await setEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), getEntry, setEntry },
		kitsuId,
		save
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return n;
}

/** Live-store wiring for "Remove from list". See [`syncSetEntry`]. */
export async function syncRemoveEntry(kitsuId: string): Promise<number> {
	const connected = accountStore.connected;
	const n = await removeEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), getEntry, removeEntry },
		kitsuId
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return n;
}
