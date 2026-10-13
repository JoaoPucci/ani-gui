// The retry plan for the detail-page live-entry read. A transient read
// used to leave the editor disabled forever showing a loading cursor;
// instead the caller retries on a schedule so a passing failure
// self-heals, and gives up (null) after a few attempts so it settles
// into an error state rather than spinning indefinitely. The driver
// that runs the plan is `list-entry-read.ts`.
//
// A rate limit is planned differently. AniList allows this machine 30
// requests a minute and answers a 429 with the seconds left in the
// window; the backend passes that on as `retry_after_secs`. Retrying
// at 1, 2, 4 and 8 seconds spends four more of the next minute's
// budget to be told the same thing, so a rate-limited failure gets one
// more attempt after the advertised wait, and a second rate limit ends
// the read.

import type { AccountApiError } from './api';
import { isRateLimit } from './set-entry-outcome';

const MAX_READ_RETRIES = 4;

/** The wait assumed when a 429 names none — half AniList's window. */
const RATE_LIMIT_DEFAULT_WAIT_MS = 30_000;
/** The band an advertised wait is honoured within: never a tight loop,
 *  never longer than the page is worth waiting on. */
const RATE_LIMIT_MIN_WAIT_MS = 1_000;
const RATE_LIMIT_MAX_WAIT_MS = 60_000;

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
 * latest last, or `null` once the read should give up.
 *
 * A plain failure follows the schedule over the plain failures so far:
 * 1 s, 2 s, 4 s, 8 s, then stop. A rate-limited failure waits what the
 * tracker advertised, clamped to one second to one minute (thirty
 * seconds when it named none), for one more attempt; a second rate
 * limit, wherever it falls, ends the read.
 */
export function planReadRetry(failures: readonly ReadFailure[]): number | null {
	const last = failures.at(-1);
	if (last?.rateLimited) {
		if (failures.slice(0, -1).some((f) => f.rateLimited)) return null;
		const advertised =
			last.retryAfterSecs === null ? RATE_LIMIT_DEFAULT_WAIT_MS : last.retryAfterSecs * 1000;
		return Math.min(RATE_LIMIT_MAX_WAIT_MS, Math.max(RATE_LIMIT_MIN_WAIT_MS, advertised));
	}
	// The schedule's step is the count of plain failures so far; before
	// the first failure, the first step.
	const plainFailures = failures.filter((f) => !f.rateLimited).length;
	if (plainFailures > MAX_READ_RETRIES) return null;
	return 1000 * 2 ** Math.max(0, plainFailures - 1);
}
