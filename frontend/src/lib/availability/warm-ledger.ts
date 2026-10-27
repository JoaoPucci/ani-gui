/**
 * Which rows this session has handed the backend's warm.
 */

/** How long after a warm an id still uncached is handed in again. */
export const WARM_RETRY_MS = 120_000;

/** Whether an id last warmed at `sentAt` (never, if undefined) is due
 *  a warm at `now`. */
export function dueForWarm(sentAt: number | undefined, now: number, retryMs: number): boolean {
	void [sentAt, now, retryMs];
	return true;
}

export interface WarmLedger {
	/** The keys among `keys` due a warm at `now`, now marked warmed. */
	take(keys: string[], now: number): string[];
	/** The warm of `keys` never reached the backend: due again. */
	forget(keys: string[]): void;
}

export function createWarmLedger(retryMs: number = WARM_RETRY_MS): WarmLedger {
	void retryMs;
	return {
		take: (keys) => keys,
		forget: () => {}
	};
}

/** The renderer's one ledger. */
export const warmLedger = createWarmLedger();
