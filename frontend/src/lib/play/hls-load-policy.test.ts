import { describe, expect, it } from 'vitest';
import { HLS_STALL_LOAD_POLICY } from './hls-load-policy';

describe('HLS_STALL_LOAD_POLICY', () => {
	it('turns a crawling fragment into a fatal in seconds, not minutes', () => {
		// One internal retry over a 15s budget: worst case ~30s to the
		// fatal that hands control to the stall machine — against the
		// engine default of two minutes per attempt across several
		// retries, which is the minutes-long black screen observed on
		// a crawling host.
		// maxTimeToFirstByteMs is hls.js 1.6's actual schema key — an
		// earlier revision wrote maxTimeToFirstByte, which an untyped
		// policy object accepted silently and the engine ignored, so
		// the first-byte bound was never configured.
		expect(HLS_STALL_LOAD_POLICY).toMatchObject({
			fragLoadPolicy: {
				default: {
					maxTimeToFirstByteMs: 10000,
					maxLoadTimeMs: 15000,
					timeoutRetry: { maxNumRetry: 1, retryDelayMs: 0, maxRetryDelayMs: 0 }
				}
			}
		});
		// Genuine transfer errors keep a modest retry ladder — they
		// are not the slow-stall shape and fail fast on their own.
		const frag = (
			HLS_STALL_LOAD_POLICY as {
				fragLoadPolicy?: { default?: { errorRetry?: { maxNumRetry?: number } } };
			}
		).fragLoadPolicy?.default?.errorRetry;
		expect(frag?.maxNumRetry).toBeGreaterThanOrEqual(1);
	});
});
