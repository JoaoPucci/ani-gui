/**
 * Stub for the seek-throttle helper. Filled in by the green commit;
 * exists at red commit time so the .test.ts imports resolve.
 */

export const SCRUBBER_SEEK_MIN_INTERVAL_MS = 100;

export function shouldThrottleSeek(
	lastSeekAt: number | null,
	now: number,
	minIntervalMs: number = SCRUBBER_SEEK_MIN_INTERVAL_MS
): boolean {
	void lastSeekAt;
	void now;
	void minIntervalMs;
	return false;
}
