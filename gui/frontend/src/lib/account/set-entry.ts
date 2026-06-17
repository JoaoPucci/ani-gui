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

/** The outcome of a multi-tracker save, split so the caller can tell a
 *  clean save (`failed === 0`) from a partial one (a connected tracker the
 *  edit didn't reach). An unmappable provider counts toward neither. */
export interface SetOutcome {
	/** Trackers the edit was written to. */
	written: number;
	/** Trackers we couldn't confirm written — a write that threw, or an
	 *  unreachable (no-bearer) provider. */
	failed: number;
}

/**
 * Write the editor's save to every connected tracker, deciding per
 * provider off that provider's live entry: send `status` only where the
 * row is missing (so it's created with the editor's status) or where the
 * user changed it; otherwise send progress alone so a tracker keeps its
 * own status. A write that throws — or a provider we can't reach (no
 * bearer) — is tallied as `failed`; an unmappable show (`setEntry` → null)
 * is neither, so the caller can require `failed === 0` before treating the
 * save as clean rather than silently leaving a connected tracker stale.
 */
export async function setEntryAcrossTrackers(
	deps: SetEntryDeps,
	kitsuId: string,
	save: EditorSave
): Promise<SetOutcome> {
	if (!kitsuId || deps.connected.length === 0) return { written: 0, failed: 0 };
	const results = await Promise.all(
		deps.connected.map(async (provider): Promise<'written' | 'failed' | 'skip'> => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return 'failed'; // connected but unreachable — can't confirm written
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				const edit = buildListEdit({
					current: current?.status ?? null,
					seededStatus: save.seededStatus,
					status: save.status,
					progress: save.progress
				});
				const res = await deps.setEntry(provider, bearer, { kitsu_id: kitsuId, ...edit });
				return res !== null ? 'written' : 'skip'; // null = unmappable → neither
			} catch {
				return 'failed';
			}
		})
	);
	return {
		written: results.filter((r) => r === 'written').length,
		failed: results.filter((r) => r === 'failed').length
	};
}

/** The outcome of a multi-tracker removal, split so the caller can tell a
 *  clean removal (`failed === 0`) from a partial one (some tracker still
 *  has the row). Absent trackers contribute to neither tally. */
export interface RemoveOutcome {
	/** Trackers that had the entry and removed it. */
	removed: number;
	/** Trackers we couldn't confirm clean — a present tracker whose delete
	 *  failed, a read that threw, or an unreachable (no-bearer) provider. */
	failed: number;
}

/**
 * Remove a show from every connected tracker that has it. Reads each
 * provider first: a provider without the row is skipped (its delete would
 * be a no-op and must not count). A present tracker whose delete fails — or
 * any provider we can't confirm clean (read threw, no bearer) — is tallied
 * as `failed`, so the caller keeps the entry visible rather than claiming a
 * clean removal while a tracker still has the row.
 */
export async function removeEntryAcrossTrackers(
	deps: RemoveEntryDeps,
	kitsuId: string
): Promise<RemoveOutcome> {
	if (!kitsuId || deps.connected.length === 0) return { removed: 0, failed: 0 };
	const results = await Promise.all(
		deps.connected.map(async (provider): Promise<'removed' | 'failed' | 'absent'> => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return 'failed'; // connected but unreachable — can't confirm clean
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				if (current === null) return 'absent'; // nothing to remove here
				await deps.removeEntry(provider, bearer, kitsuId);
				return 'removed';
			} catch {
				return 'failed';
			}
		})
	);
	return {
		removed: results.filter((r) => r === 'removed').length,
		failed: results.filter((r) => r === 'failed').length
	};
}

/**
 * Live-store wiring: fan the editor's save out to every connected tracker
 * off the account store, refreshing a near-expiry bearer first, then
 * invalidate the Watch Later snapshot (a status change can cross the
 * planning boundary). Returns the success count for toast feedback.
 */
export async function syncSetEntry(kitsuId: string, save: EditorSave): Promise<SetOutcome> {
	const connected = accountStore.connected;
	const outcome = await setEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), getEntry, setEntry },
		kitsuId,
		save
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return outcome;
}

/** Live-store wiring for "Remove from list". See [`syncSetEntry`]. Returns
 *  the split outcome so the editor only reports a clean removal when no
 *  tracker that had the row was left behind. */
export async function syncRemoveEntry(kitsuId: string): Promise<RemoveOutcome> {
	const connected = accountStore.connected;
	const outcome = await removeEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), getEntry, removeEntry },
		kitsuId
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return outcome;
}
