import { describe, expect, it } from 'vitest';
import {
	classifyStallCause,
	exhaustedStallOverlayMessage,
	stallNudgeToast,
	stallRecoveryToast
} from './stall-notice';
import { m } from '$lib/paraglide/messages';

describe('classifyStallCause', () => {
	it('reads hls timeout and stall details as the host struggling', () => {
		// The playlists loaded — hls.js got far enough to time out on
		// media — while segments crawl: the provider-shard signature.
		expect(classifyStallCause('hls fragLoadTimeOut')).toBe('host-slow');
		expect(classifyStallCause('hls levelLoadTimeOut')).toBe('host-slow');
		expect(classifyStallCause('hls bufferStalledError')).toBe('host-slow');
	});

	it('reads everything else network-class as a rotated link', () => {
		expect(classifyStallCause('hls fragLoadError')).toBe('link-stale');
		expect(classifyStallCause('video MEDIA_ERR_NETWORK')).toBe('link-stale');
	});
});

describe('stallRecoveryToast', () => {
	it('names the stall when the host is struggling', () => {
		expect(stallRecoveryToast('hls fragLoadTimeOut')).toEqual({
			kind: 'info',
			message: m.play_stall_toast_host_slow(),
			duration: 8000
		});
	});

	it('names the expired link otherwise', () => {
		expect(stallRecoveryToast('video MEDIA_ERR_NETWORK (net err)')).toEqual({
			kind: 'info',
			message: m.play_stall_toast_link_stale(),
			duration: 8000
		});
	});

	it('stays quiet for a manual reload — the user started it', () => {
		expect(stallRecoveryToast('manual reload')).toBeNull();
	});
});

describe('exhaustedStallOverlayMessage', () => {
	it('names the slow host once the retry budget is spent', () => {
		expect(
			exhaustedStallOverlayMessage(
				{ source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' },
				true
			)
		).toBe(m.play_error_host_slow());
	});

	it('keeps the generic message before the silent retry ran', () => {
		// The first timeout goes to the silent recovery, not the
		// overlay; if this fires the caller is on a different path.
		expect(
			exhaustedStallOverlayMessage(
				{ source: 'hls', type: 'networkError', details: 'fragLoadTimeOut' },
				false
			)
		).toBeNull();
	});

	it('keeps the generic message for non-timeout and non-network failures', () => {
		expect(
			exhaustedStallOverlayMessage(
				{ source: 'hls', type: 'networkError', details: 'fragLoadError' },
				true
			)
		).toBeNull();
		expect(exhaustedStallOverlayMessage({ source: 'hls', type: 'mediaError' }, true)).toBeNull();
		expect(exhaustedStallOverlayMessage({ source: 'video', code: 2 }, true)).toBeNull();
	});
});

describe('stallNudgeToast', () => {
	it('says the host is slow and the SAME stream is being retried', () => {
		// Doubled duration on every stall toast: a 4s flash over a
		// black frame is a signal nobody sees.
		// The recovery toasts promise a fresh link; a nudge retries
		// the stream it already has, so that wording would lie.
		expect(stallNudgeToast()).toEqual({
			kind: 'info',
			message: m.play_stall_toast_nudge(),
			duration: 8000
		});
	});
});
