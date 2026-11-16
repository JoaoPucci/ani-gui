// Whether Kitsu answered that an entry is gone — split from match.ts so
// the stored-mapping step ($lib/history/match-stored.ts) can tell it
// from a failure without importing the resolver that imports it.

/** Whether a failed Kitsu read is Kitsu answering that the entry is
 *  gone — a 404 or 410, which the backend passes on as an upstream
 *  error carrying the status — rather than a failure that says nothing
 *  about the id (the network, a 5xx, a rate limit, a timeout). */
export function kitsuEntryGone(e: unknown): boolean {
	const err = e as { kind?: unknown; status?: unknown } | null;
	return err?.kind === 'upstream' && (err.status === 404 || err.status === 410);
}
