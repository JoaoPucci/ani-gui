/**
 * Freshness policy for the home Watch Later rail.
 *
 * The rail renders from a local snapshot of each provider's
 * Plan-to-Watch list (`user_list_cache`), which is only re-pulled
 * from the tracker on connect. To pick up entries added on the
 * provider's website without forcing a re-connect, the rail
 * re-pulls in the background when its snapshot is older than a TTL
 * (and on a manual refresh). "When did we last pull?" is tracked
 * per provider in localStorage so the TTL survives app restarts.
 */

import type { Provider } from './types';

/** How long a cached snapshot is considered fresh (6 hours). */
export const WATCH_LATER_TTL_MS = 6 * 60 * 60 * 1000;

const KEY_PREFIX = 'aniGui:watchLater:lastRefresh:';

/**
 * True when the snapshot should be re-pulled: never refreshed
 * (`null`) or older than `ttlMs`. Pure so the policy is unit-tested
 * without touching storage or the clock.
 */
export function isWatchLaterStale(
	lastRefreshedMs: number | null,
	nowMs: number,
	ttlMs: number = WATCH_LATER_TTL_MS
): boolean {
	if (lastRefreshedMs == null) return true;
	return nowMs - lastRefreshedMs >= ttlMs;
}

/** Read the last-refresh epoch (ms) for a provider, or null if unset
 *  / unparseable. Mirrors the defensive localStorage reads elsewhere
 *  (`topbar/dropdown.ts`). */
export function readLastRefreshed(provider: Provider): number | null {
	try {
		const raw = window.localStorage.getItem(KEY_PREFIX + provider);
		if (raw == null) return null;
		const n = Number.parseInt(raw, 10);
		return Number.isFinite(n) ? n : null;
	} catch {
		return null;
	}
}

/** Record that the provider's snapshot was just re-pulled. */
export function markRefreshed(provider: Provider, nowMs: number): void {
	try {
		window.localStorage.setItem(KEY_PREFIX + provider, String(nowMs));
	} catch {
		/* storage unavailable (private mode etc.) — TTL just falls back
		   to refreshing each launch, which is acceptable. */
	}
}

/** Force the provider's snapshot to be treated as stale on the next
 *  rail load. Called after a local mark-watched sync, which can move a
 *  Plan-to-Watch title to Watching (dropping it from the rail) — the
 *  re-pull reflects that without waiting out the TTL. */
export function invalidateWatchLater(provider: Provider): void {
	invalidations.set(provider, invalidatedSoFar(provider) + 1);
	try {
		window.localStorage.removeItem(KEY_PREFIX + provider);
	} catch {
		/* storage unavailable — next launch re-pulls anyway. */
	}
}

/** How many times each provider's snapshot was invalidated this session. */
const invalidations = new Map<Provider, number>();

function invalidatedSoFar(provider: Provider): number {
	return invalidations.get(provider) ?? 0;
}

/** Noted when a refresh of `provider` starts, for {@link markRefreshedSince}. */
export function refreshStamp(provider: Provider): number {
	return invalidatedSoFar(provider);
}

/**
 * Record a refresh of `provider` that started at `stamp` as the latest
 * pull — unless the snapshot was invalidated since: a write that settled
 * while the refresh was out asked for a read the refresh started too
 * early to be, so the snapshot stays due. Returns whether it was marked.
 */
export function markRefreshedSince(provider: Provider, stamp: number, nowMs: number): boolean {
	if (invalidatedSoFar(provider) !== stamp) return false;
	markRefreshed(provider, nowMs);
	return true;
}
