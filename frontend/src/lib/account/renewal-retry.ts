/**
 * Retrying a renewal the tracker did not answer.
 *
 * A sign-in whose renewal failed only because the tracker could not be
 * reached may still be valid, so the app tries again on its own rather
 * than leaving it to the next launch.
 */

import type { Provider, ProviderState } from './types';
import type { RenewalFailures } from './renewal-failure';

/** Whether some provider is waiting on a renewal the tracker did not answer. */
export function needsRenewalRetry(
	_byProvider: Record<Provider, ProviderState>,
	_failures: RenewalFailures
): boolean {
	void _byProvider;
	void _failures;
	return false;
}

/** How long to wait before the given retry (0 is the first). */
export function retryDelayMs(_attempt: number): number {
	void _attempt;
	return 0;
}

export interface RenewalRetryDeps {
	needsRetry(): boolean;
	retry(): Promise<void>;
	setTimer(fn: () => void, ms: number): unknown;
	clearTimer(handle: unknown): void;
}

/** Keeps one retry scheduled while a renewal is waiting on the tracker. */
export class RenewalRetry {
	constructor(private readonly deps: RenewalRetryDeps) {}

	sync(): void {
		void this.deps;
	}
}
