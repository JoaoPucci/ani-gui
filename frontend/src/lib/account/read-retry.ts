// The retry plan for the detail-page live-entry read. A transient read
// used to leave the editor disabled forever showing a loading cursor;
// instead the caller retries on a schedule so a passing failure
// self-heals, and gives up (null) after a few attempts so it settles
// into an error state rather than spinning indefinitely. The driver
// that runs the plan is `list-entry-read.ts`.

import type { AccountApiError } from './api';
import { isRateLimit } from './set-entry-outcome';

const MAX_READ_RETRIES = 4;

/** One failed attempt, as the plan sees it. */
export interface ReadFailure {
	/** The tracker refused for a while — the backend answered HTTP 429. */
	rateLimited: boolean;
	/** The wait the tracker advertised, in seconds, when the envelope
	 *  carried one (`retry_after_secs`). */
	retryAfterSecs: number | null;
	/** The backend's error envelope, when the body was one. */
	envelope: Record<string, unknown> | null;
}

/** The body of an account error as the backend's envelope, or null
 *  when it is not a JSON object. */
function envelopeOf(detail: string): Record<string, unknown> | null {
	try {
		const parsed: unknown = JSON.parse(detail);
		return typeof parsed === 'object' && parsed !== null
			? (parsed as Record<string, unknown>)
			: null;
	} catch {
		return null;
	}
}

/** Read a failed attempt: a 429 with whatever wait its envelope
 *  carries; anything else — a transport failure, a missing bearer,
 *  another status — as a plain failure. */
export function classifyReadFailure(e: unknown): ReadFailure {
	if (!isRateLimit(e)) return { rateLimited: false, retryAfterSecs: null, envelope: null };
	const envelope = envelopeOf((e as AccountApiError).detail);
	const secs = envelope?.retry_after_secs;
	return {
		rateLimited: true,
		retryAfterSecs: typeof secs === 'number' ? secs : null,
		envelope
	};
}

/**
 * Delay (ms) before the next attempt given every failure so far, the
 * latest last, or `null` once the read should give up: 1 s, 2 s, 4 s,
 * 8 s, then stop.
 */
export function planReadRetry(failures: readonly ReadFailure[]): number | null {
	const attempt = failures.length - 1;
	if (attempt >= MAX_READ_RETRIES) return null;
	return 1000 * 2 ** attempt;
}
