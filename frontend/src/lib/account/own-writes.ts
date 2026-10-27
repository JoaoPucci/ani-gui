/**
 * The bearers a renewal of this renderer is writing to the keychain
 * right now, per provider: from just before its save is queued until
 * that save settles. A re-read of the keychain that finds one of them
 * has found the renewal's own write, not a different sign-in.
 */

import type { Provider } from './types';

const writing: Record<Provider, Map<string, number>> = {
	anilist: new Map(),
	mal: new Map(),
	inhouse: new Map()
};

/** Record that a save of `bearer` for `provider` is under way; the
 *  returned function ends it. */
export function beginOwnWrite(provider: Provider, bearer: string): () => void {
	const m = writing[provider];
	m.set(bearer, (m.get(bearer) ?? 0) + 1);
	let ended = false;
	return () => {
		if (ended) return;
		ended = true;
		const n = (m.get(bearer) ?? 1) - 1;
		if (n <= 0) m.delete(bearer);
		else m.set(bearer, n);
	};
}

/** The bearers being written for `provider` now. */
export function ownWrites(provider: Provider): ReadonlySet<string> {
	return new Set(writing[provider].keys());
}
