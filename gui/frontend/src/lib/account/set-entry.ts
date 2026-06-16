// Fan-out for the detail-page list editor: push an explicit edit (or a
// removal) to every connected tracker. Renderer-driven — the backend is
// stateless, so the renderer holds the per-provider bearers and
// orchestrates the calls (mirrors push-watched.ts).

import type { ListEntry, ListStatus, Provider } from './types';
import { removeEntry, setEntry } from './api';
import { accountStore } from './store.svelte';
import { freshBearerFor } from './fresh-bearer';
import { invalidateWatchLater } from './watch-later-refresh';

/** A deliberate edit from the detail-page editor. Absent fields are left
 *  unchanged on the tracker. */
export interface ListEdit {
	status?: ListStatus;
	progress?: number;
}

export interface SetEntryDeps {
	/** Providers currently connected (from the account store). */
	connected: Provider[];
	/** Resolve a provider's bearer, or null if unavailable. May be async
	 *  so the caller can refresh a near-expiry token first. */
	bearerFor: (provider: Provider) => string | null | Promise<string | null>;
	/** POST the explicit edit to one provider. */
	setEntry: (
		provider: Provider,
		bearer: string,
		body: { kitsu_id: string; status?: string; progress?: number }
	) => Promise<ListEntry | null>;
}

export interface RemoveEntryDeps {
	connected: Provider[];
	bearerFor: (provider: Provider) => string | null | Promise<string | null>;
	removeEntry: (provider: Provider, bearer: string, kitsuId: string) => Promise<void>;
}

/**
 * Write an explicit edit to every connected tracker. Best-effort per
 * provider — a missing bearer is skipped, and an unmappable show (the
 * backend replies `null`) or a thrown error counts as not-written
 * without blocking the others. Returns how many providers accepted the
 * write, so the caller can toast success vs. "couldn't update".
 */
export async function setEntryAcrossTrackers(
	deps: SetEntryDeps,
	kitsuId: string,
	edit: ListEdit
): Promise<number> {
	if (!kitsuId || deps.connected.length === 0) return 0;
	const body: { kitsu_id: string; status?: string; progress?: number } = { kitsu_id: kitsuId };
	if (edit.status !== undefined) body.status = edit.status;
	if (edit.progress !== undefined) body.progress = edit.progress;
	const results = await Promise.all(
		deps.connected.map(async (provider) => {
			const bearer = await deps.bearerFor(provider);
			if (!bearer) return false;
			try {
				return (await deps.setEntry(provider, bearer, body)) !== null;
			} catch {
				return false;
			}
		})
	);
	return results.filter(Boolean).length;
}

/**
 * Remove a show from every connected tracker. Best-effort, returns the
 * count removed (the backend's delete is idempotent, so an
 * already-removed title still resolves successfully).
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
 * Live-store wiring: fan an explicit edit out to every connected tracker
 * off the account store, refreshing a near-expiry bearer first, then
 * invalidate the Watch Later snapshot (a status change can cross the
 * planning boundary). Returns the success count for toast feedback.
 */
export async function syncSetEntry(kitsuId: string, edit: ListEdit): Promise<number> {
	const connected = accountStore.connected;
	const n = await setEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), setEntry },
		kitsuId,
		edit
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return n;
}

/** Live-store wiring for "Remove from list". See [`syncSetEntry`]. */
export async function syncRemoveEntry(kitsuId: string): Promise<number> {
	const connected = accountStore.connected;
	const n = await removeEntryAcrossTrackers(
		{ connected, bearerFor: (provider) => freshBearerFor(provider), removeEntry },
		kitsuId
	);
	for (const provider of connected) invalidateWatchLater(provider);
	return n;
}
