// Fan-out outcome folding for the detail-page list editor, split out of
// `set-entry.ts` so that file's per-file CRAP stays under the ratchet (the
// 429/rate-limit branches pushed it over). Pure + unit-tested.

import { AccountApiError } from './api';

/** True when a failed account call was a 429 (rate limit) — lets the fan-out
 *  flag rate-limiting so the editor prompts a retry instead of a generic
 *  failure. */
export function isRateLimit(error: unknown): boolean {
	return error instanceof AccountApiError && error.status === 429;
}

/** Per-provider fan-out result: the call landed (`ok`), threw (`failed`),
 *  was rate-limited (`ratelimited`), or didn't apply — unmappable show or a
 *  tracker that didn't have the row (`neither`). */
export type FanoutResult = 'ok' | 'failed' | 'ratelimited' | 'neither';

export interface FanoutTally {
	/** Providers the call landed on. */
	ok: number;
	/** Providers that threw or rate-limited — couldn't be confirmed. A 429
	 *  still counts here (nothing was written); `rateLimited` distinguishes it. */
	failed: number;
	/** At least one provider rejected with a 429. */
	rateLimited: boolean;
}

/** Fold per-provider results into counts + a rate-limit flag. */
export function tallyFanout(results: FanoutResult[]): FanoutTally {
	return {
		ok: results.filter((r) => r === 'ok').length,
		failed: results.filter((r) => r === 'failed' || r === 'ratelimited').length,
		rateLimited: results.includes('ratelimited')
	};
}
