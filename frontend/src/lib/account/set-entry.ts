// Fan-out for the detail-page list editor: push an explicit edit (or a
// removal) to every connected tracker. Renderer-driven — the backend is
// stateless, so the renderer holds the per-provider bearers and
// orchestrates the calls (mirrors push-watched.ts).

import type { EntryView, ListEntry, ListStatus, Provider } from './types';
import { getEntry, removeEntry, setEntry } from './entry-api';
import { isRateLimit, tallyFanout, type FanoutResult } from './set-entry-outcome';
import { buildListEdit } from './list-entry-edit';
import { accountStore } from './store.svelte';
import { freshBearerFor } from './fresh-bearer';
import { invalidateWatchLater } from './watch-later-refresh';

/**
 * A deliberate edit from the detail-page editor. `status` is the value shown
 * in the editor (what the user picked); `statusChanged` says whether the user
 * deliberately set status this session — moved it off the opened-on value, OR
 * a pending partial-save retry is active. When false the fan-out leaves each
 * tracker's own status alone and writes progress only, so a progress-only edit
 * doesn't converge a divergent status.
 */
export interface EditorSave {
	status: ListStatus;
	statusChanged: boolean;
	progress: number;
	/** The show's episode total, so a completed save snaps to the full count
	 *  per provider. Null when unknown (ongoing show). */
	total?: number | null;
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
	/** Present (and `true`) when at least one connected tracker rejected the
	 *  call with a 429 (rate limit). Lets the editor show a "rate-limited, try
	 *  again" toast instead of the generic failure copy. */
	rateLimited?: boolean;
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
		deps.connected.map(async (provider): Promise<FanoutResult> => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return 'failed'; // connected but unreachable — can't confirm written
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				const edit = buildListEdit({
					current: current?.status ?? null,
					statusChanged: save.statusChanged,
					status: save.status,
					progress: save.progress,
					total: save.total ?? null
				});
				const res = await deps.setEntry(provider, bearer, { kitsu_id: kitsuId, ...edit });
				return res !== null ? 'ok' : 'neither'; // null = unmappable → neither
			} catch (e) {
				// A 429 is still a failure (nothing was written), but flagged so the
				// editor can say "rate-limited" rather than a generic error.
				return isRateLimit(e) ? 'ratelimited' : 'failed';
			}
		})
	);
	const t = tallyFanout(results);
	return { written: t.ok, failed: t.failed, ...(t.rateLimited ? { rateLimited: true } : {}) };
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
	/** Present (and `true`) when a connected tracker rejected a call with a 429
	 *  (rate limit) — see {@link SetOutcome.rateLimited}. */
	rateLimited?: boolean;
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
		deps.connected.map(async (provider): Promise<FanoutResult> => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return 'failed'; // connected but unreachable — can't confirm clean
			try {
				const current = await deps.getEntry(provider, bearer, kitsuId);
				if (current === null) return 'neither'; // nothing to remove here
				await deps.removeEntry(provider, bearer, kitsuId);
				return 'ok';
			} catch (e) {
				return isRateLimit(e) ? 'ratelimited' : 'failed';
			}
		})
	);
	const t = tallyFanout(results);
	return { removed: t.ok, failed: t.failed, ...(t.rateLimited ? { rateLimited: true } : {}) };
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
