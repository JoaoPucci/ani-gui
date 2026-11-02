// Clearing a stale cached row locally, under the user a removal was made
// for. Kept out of `set-entry.ts` so that file stays inside the CRAP
// gate's per-file bar.

import { forgetCachedEntry } from './entry-api';
import type { Provider } from './types';

/**
 * Clear a show's cached row under the user each tracker was listed under
 * when the removal was fanned out; nothing for a tracker listed under no
 * one.
 */
export function forgetCachedUnder(
	listed: Readonly<Record<string, { user: string | null | undefined }>>
): (provider: Provider, kitsuId: string) => Promise<void> {
	return (provider, kitsuId) => {
		const user = listed[provider]?.user;
		return user ? forgetCachedEntry(provider, kitsuId, user) : Promise.resolve();
	};
}
