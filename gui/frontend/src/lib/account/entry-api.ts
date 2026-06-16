/**
 * Detail-page list-entry editor endpoints — read the live entry, write an
 * explicit edit, remove a show. Split out of `api.ts` so that file's
 * HTTP-call surface keeps its CCN under the coverage ratchet; these build
 * on the shared `apiBase`/`postJson`/`deleteEndpoint` primitives there.
 */

import { AccountApiError, apiBase, deleteEndpoint, postJson, readErrorBody } from './api';
import type { EntryView, ListEntry, Provider } from './types';

async function getJson<T>(path: string, bearer: string): Promise<T> {
	const base = await apiBase();
	const res = await fetch(base.replace(/\/+$/, '') + path, {
		headers: { authorization: `Bearer ${bearer}` }
	});
	if (!res.ok) {
		throw new AccountApiError(res.status, await readErrorBody(res));
	}
	return (await res.json()) as T;
}

/**
 * Read the user's live current list entry for a show so the detail-page
 * editor opens on the real tracker state (the deviation safety). `null`
 * when the show isn't on the list or isn't mapped to the provider.
 */
export function getEntry(
	provider: Provider,
	bearer: string,
	kitsuId: string
): Promise<EntryView | null> {
	return getJson<EntryView | null>(
		`/api/account/entry/${provider}?kitsu_id=${encodeURIComponent(kitsuId)}`,
		bearer
	);
}

/**
 * Write an explicit list edit (status and/or progress) — the detail-page
 * editor. The backend writes it verbatim (no monotonic guard), so a
 * downward episode correction goes through. Returns the upserted entry,
 * or `null` when the show couldn't be mapped to the provider.
 */
export function setEntry(
	provider: Provider,
	bearer: string,
	body: { kitsu_id: string; status?: string; progress?: number }
): Promise<ListEntry | null> {
	return postJson<ListEntry | null>(`/api/account/set/${provider}`, body, bearer);
}

/**
 * Remove a show from the user's tracker list (editor "Remove"). The
 * backend is idempotent — an already-removed title still resolves
 * successfully.
 */
export function removeEntry(provider: Provider, bearer: string, kitsuId: string): Promise<void> {
	return deleteEndpoint(
		`/api/account/entry/${provider}?kitsu_id=${encodeURIComponent(kitsuId)}`,
		bearer
	);
}
