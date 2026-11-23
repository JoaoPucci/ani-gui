// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import Hls from 'hls.js';
import { HLS_BUFFER_POLICY } from './hls-buffer-policy';
import { PLAYER_SEEK_STEP_SECONDS } from './keyboard';

describe('HLS_BUFFER_POLICY', () => {
	it('lets already-played video go: the back buffer is bounded, at the 30 s mature players keep', () => {
		// hls.js keeps everything behind the playhead by default
		// (backBufferLength: Infinity), until Chromium's own per-source
		// cap evicts the oldest. Measured on a 1080p stream, five minutes
		// in, the media pipeline held four and a half minutes of video,
		// all but the last minute and a half reachable only by seeking
		// back.
		expect(HLS_BUFFER_POLICY.backBufferLength).toBe(30);
	});

	it('keeps several arrow-key seeks back without asking the host again', () => {
		expect(HLS_BUFFER_POLICY.backBufferLength).toBeGreaterThanOrEqual(4 * PLAYER_SEEK_STEP_SECONDS);
	});

	it("leaves the forward buffer to the engine's byte-bounded default", () => {
		// What lies ahead is the runway the stall handling waits on, and
		// the engine already bounds it by size (60 MB) as well as by
		// time; the policy does not touch it.
		const engine = new Hls({ ...HLS_BUFFER_POLICY });
		try {
			expect(engine.config.backBufferLength).toBe(30);
			expect(engine.config.maxBufferSize).toBe(Hls.DefaultConfig.maxBufferSize);
			expect(engine.config.maxBufferLength).toBe(Hls.DefaultConfig.maxBufferLength);
			expect(engine.config.maxMaxBufferLength).toBe(Hls.DefaultConfig.maxMaxBufferLength);
			expect(Number.isFinite(engine.config.maxBufferSize)).toBe(true);
		} finally {
			engine.destroy();
		}
	});
});
