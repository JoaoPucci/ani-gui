/**
 * Modal-copy selector for the "ffmpeg is missing" failure. The
 * original modal body assumed a Windows installer flow ("re-run the
 * installer with an internet connection") and the action button
 * linked to ffmpeg.org/download — both fine on Windows, neither
 * helpful on Linux or macOS where the recovery path is one
 * package-manager command. This helper maps `process.platform`
 * (surfaced via `window.aniGui.platform`) to a body-key + show-action
 * pair so the layout can pick the right `m.*` message and decide
 * whether to render the external-link button.
 *
 * Pure / framework-agnostic — i18n resolution lives at the call
 * site so this module stays trivially testable.
 */
import { describe, it, expect } from 'vitest';
import { selectFfmpegMissingCopy } from './ffmpeg-missing-copy';

describe('selectFfmpegMissingCopy', () => {
	it('returns the Windows body with the ffmpeg.org action on win32', () => {
		expect(selectFfmpegMissingCopy('win32')).toEqual({
			bodyKey: 'win32',
			showAction: true
		});
	});

	it('returns the macOS body with the ffmpeg.org action on darwin (covers users without Homebrew)', () => {
		expect(selectFfmpegMissingCopy('darwin')).toEqual({
			bodyKey: 'darwin',
			showAction: true
		});
	});

	it('returns the Linux body and suppresses the action button — the inline package-manager commands are the recovery path', () => {
		expect(selectFfmpegMissingCopy('linux')).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
	});

	it('falls back to the Linux body for unknown platforms (FreeBSD / OpenBSD / etc.) — package-manager culture is closer to Linux than Windows', () => {
		expect(selectFfmpegMissingCopy('freebsd')).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
		expect(selectFfmpegMissingCopy('openbsd')).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
	});

	it('falls back to the Linux body when the platform string is missing — preload bridge could be absent in tests or under contextIsolation edge cases', () => {
		expect(selectFfmpegMissingCopy(undefined)).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
		expect(selectFfmpegMissingCopy(null)).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
		expect(selectFfmpegMissingCopy('')).toEqual({
			bodyKey: 'linux',
			showAction: false
		});
	});
});
