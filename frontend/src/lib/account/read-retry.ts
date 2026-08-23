// Backoff schedule for the detail-page live-entry read. A rate-limited (429)
// or transient read used to leave the editor disabled forever showing a
// loading cursor; instead the caller retries on this schedule so a passing
// rate-limit self-heals, and gives up (null) after a few attempts so it
// settles into an error state rather than spinning indefinitely.

const MAX_READ_RETRIES = 4;

/** Delay (ms) before retry `attempt` (0-based), or `null` once exhausted. */
export function nextReadRetryMs(attempt: number): number | null {
	if (attempt >= MAX_READ_RETRIES) return null;
	return 1000 * 2 ** attempt; // 1s, 2s, 4s, 8s
}
