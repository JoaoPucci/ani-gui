/**
 * Load-policy overrides for the player engine.
 *
 * hls.js ships extremely patient defaults — a fragment may take two
 * minutes to load and is retried several times before the fatal our
 * stall handling reacts to, which put every stall surface (toast,
 * recovery, cause-naming overlay) minutes behind a black screen on a
 * crawling host. These overrides make a dead-crawling stream turn
 * into a fatal in seconds: a healthy segment loads well inside the
 * budget, and the stall machine — not the engine's internal retry
 * loop — owns what happens next.
 */

import type { HlsConfig } from 'hls.js';

/** Typed against hls.js's own configuration so a schema typo is a
 *  compile error — an untyped object silently accepted a misspelled
 *  key once, and the engine ignored it. */
export const HLS_STALL_LOAD_POLICY: Pick<HlsConfig, 'fragLoadPolicy'> = {
	fragLoadPolicy: {
		default: {
			maxTimeToFirstByteMs: 10000,
			maxLoadTimeMs: 15000,
			timeoutRetry: { maxNumRetry: 1, retryDelayMs: 0, maxRetryDelayMs: 0 },
			errorRetry: { maxNumRetry: 3, retryDelayMs: 1000, maxRetryDelayMs: 4000 }
		}
	}
};
