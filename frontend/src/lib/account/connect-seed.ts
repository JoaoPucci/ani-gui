// The list a freshly connected tracker account is seeded with. Kept out
// of the account page so the page stays a thin adapter (AGENTS.md §2).

import { fetchAndCacheList } from './api';
import { markRefreshed } from './watch-later-refresh';
import type { Provider } from './types';

/**
 * Pull `provider`'s full list into the backend's cache right after a
 * connect, for the rails and account stats that read it, and stamp it
 * fresh so Home's freshness check does not pull it again at once.
 * Best-effort: the connection is valid either way, and a failed pull
 * leaves the cache for the next rail mount to warm.
 */
export async function seedConnectedList(provider: Provider, bearer: string): Promise<void> {
	try {
		await fetchAndCacheList(provider, bearer);
		markRefreshed(provider, Date.now());
	} catch {
		/* non-fatal — next mount warms the cache */
	}
}
