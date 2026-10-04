/**
 * Re-asking the schedule when a user clicks an episode the page calls
 * unaired. Signatures only: the behaviour lands with the change these
 * tests describe.
 */

export const UNAIRED_RECHECK_INTERVAL_MS = 5 * 60_000;

export type UnairedRecheckOutcome = 'aired' | 'unaired' | 'failed' | 'superseded';

export interface UnairedRecheckDeps {
	refresh: () => Promise<void>;
	isAired: () => boolean;
	currentContext: () => string;
	now: () => number;
}

export function createUnairedRecheck(intervalMs: number = UNAIRED_RECHECK_INTERVAL_MS): {
	check: (showId: string, deps: UnairedRecheckDeps) => Promise<UnairedRecheckOutcome>;
} {
	void intervalMs;
	return { check: async () => 'unaired' };
}

export const unairedRecheck = createUnairedRecheck();

export type UnairedClickPlan = 'play' | 'recheck-provider' | 'still-unaired' | 'failed' | 'none';

export function planUnairedClick(
	outcome: UnairedRecheckOutcome,
	beyondPlayable: boolean
): UnairedClickPlan {
	void outcome;
	void beyondPlayable;
	return 'none';
}
