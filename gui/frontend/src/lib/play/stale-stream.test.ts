import { describe, expect, it } from 'vitest';
import { shouldAttemptStaleStreamRetry } from './stale-stream';

describe('shouldAttemptStaleStreamRetry', () => {
	it('allows the first auto-retry for a <video> network error (code 2)', () => {
		// Upstream URL rotated while the user was idle → the <video>
		// element fires `error` with code=2 the moment they click play.
		// The first such error should kick the silent evict + refetch.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 2 },
				hasAutoRetried: false
			})
		).toBe(true);
	});

	it('allows the first auto-retry for an hls.js fatal networkError', () => {
		// HLS sessions take a different code path (Hls.Events.ERROR with
		// data.type === 'networkError'). Same root cause as the <video>
		// code-2 case; same retry policy.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'hls', type: 'networkError' },
				hasAutoRetried: false
			})
		).toBe(true);
	});

	it('blocks a second auto-retry in the same session (one-shot guard)', () => {
		// Critical loop guard. If the silent retry itself lands on a
		// fresh-but-also-broken URL, the second error must surface to
		// the user instead of re-triggering eviction forever.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 2 },
				hasAutoRetried: true
			})
		).toBe(false);
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'hls', type: 'networkError' },
				hasAutoRetried: true
			})
		).toBe(false);
	});

	it('does not retry video decode errors (code 3)', () => {
		// MEDIA_ERR_DECODE — corrupt byte stream or codec mismatch.
		// Refetching the same URL won't help; evicting cached resolution
		// data is wasted IPC. Surface to the user instead.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 3 },
				hasAutoRetried: false
			})
		).toBe(false);
	});

	it('does not retry video not-supported errors (code 4)', () => {
		// MEDIA_ERR_SRC_NOT_SUPPORTED — the webview can't decode this
		// container/codec at all. Same reasoning as decode: not a
		// rotated-URL symptom.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 4 },
				hasAutoRetried: false
			})
		).toBe(false);
	});

	it('does not retry video aborted errors (code 1)', () => {
		// MEDIA_ERR_ABORTED — user-initiated abort. Not an error worth
		// auto-retrying.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 1 },
				hasAutoRetried: false
			})
		).toBe(false);
	});

	it('does not retry non-network HLS errors', () => {
		// Media-pipeline / muxer / parser errors aren't URL-rotation
		// symptoms either.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'hls', type: 'mediaError' },
				hasAutoRetried: false
			})
		).toBe(false);
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'hls', type: 'otherError' },
				hasAutoRetried: false
			})
		).toBe(false);
	});

	it('treats an unknown video code (0) as non-retryable', () => {
		// Belt-and-braces: only the explicit MEDIA_ERR_NETWORK case
		// (code === 2) is treated as a rotated-URL signal.
		expect(
			shouldAttemptStaleStreamRetry({
				err: { source: 'video', code: 0 },
				hasAutoRetried: false
			})
		).toBe(false);
	});
});
