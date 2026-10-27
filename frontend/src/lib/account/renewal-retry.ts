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
	byProvider: Record<Provider, ProviderState>,
	failures: RenewalFailures
): boolean {
	return (Object.keys(byProvider) as Provider[]).some((p) => {
		const s = byProvider[p];
		return s.kind === 'expired' && !!s.account.refresh_token && failures[p] === 'unreachable';
	});
}

const FIRST_RETRY_MS = 30_000;
const LONGEST_RETRY_MS = 600_000;

/** How long to wait before the given retry (0 is the first): thirty
 *  seconds, doubling, never more than ten minutes. */
export function retryDelayMs(attempt: number): number {
	const doublings = Math.min(Math.max(0, attempt), 10);
	return Math.min(FIRST_RETRY_MS * 2 ** doublings, LONGEST_RETRY_MS);
}

export interface RenewalRetryDeps {
	needsRetry(): boolean;
	retry(): Promise<void>;
	setTimer(fn: () => void, ms: number): unknown;
	clearTimer(handle: unknown): void;
}

/** Keeps one retry scheduled while a renewal is waiting on the tracker. */
export class RenewalRetry {
	private timer: unknown = null;
	private running = false;
	private attempt = 0;

	constructor(private readonly deps: RenewalRetryDeps) {}

	/** Look again: schedule a retry if one is due and none is pending,
	 *  or cancel the pending one once nothing waits. */
	sync(): void {
		if (!this.deps.needsRetry()) {
			if (this.timer !== null) this.deps.clearTimer(this.timer);
			this.timer = null;
			this.attempt = 0;
			return;
		}
		if (this.timer !== null || this.running) return;
		this.timer = this.deps.setTimer(() => this.fire(), retryDelayMs(this.attempt));
		this.attempt += 1;
	}

	/** Stop for good. Stub: does nothing yet. */
	stop(): void {}

	private fire(): void {
		this.timer = null;
		this.running = true;
		void this.deps.retry().finally(() => {
			this.running = false;
			this.sync();
		});
	}
}
