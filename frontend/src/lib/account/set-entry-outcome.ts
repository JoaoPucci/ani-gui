// Fan-out outcome folding for the detail-page list editor, split out of
// `set-entry.ts` so that file's per-file CRAP stays under the ratchet (the
// 429/rate-limit branches pushed it over). Pure + unit-tested.

/** True when a failed account call was a 429 (rate limit) — lets the fan-out
 *  flag rate-limiting so the editor prompts a retry instead of a generic
 *  failure. */
export function isRateLimit(error: unknown): boolean {
	// By shape: an AccountApiError carries `status`, and a test that mocks
	// the api module hands the fan-out errors that are not its instances.
	return error instanceof Error && (error as Error & { status?: unknown }).status === 429;
}

/** The per-provider result a thrown write stands for. */
export function failureResult(error: unknown, superseded: boolean): FanoutResult {
	if (superseded) return 'superseded';
	return isRateLimit(error) ? 'ratelimited' : 'failed';
}

/** The flags a fan-out's results add to its outcome, present only when
 *  true: a 429 (`rateLimited`), and a write superseded by a newer one of
 *  the same show — neither written nor failed (`superseded`). */
export function outcomeFlags(results: FanoutResult[]): {
	rateLimited?: true;
	superseded?: true;
} {
	return {
		...(results.includes('ratelimited') ? { rateLimited: true as const } : {}),
		...(results.includes('superseded') ? { superseded: true as const } : {})
	};
}

/** Per-provider fan-out result: the call landed (`ok`), threw (`failed`),
 *  was rate-limited (`ratelimited`), was dropped for a newer write of the
 *  same show while it waited (`superseded`), or didn't apply — unmappable
 *  show or a tracker that didn't have the row (`neither`). */
export type FanoutResult = 'ok' | 'failed' | 'ratelimited' | 'superseded' | 'neither';

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
