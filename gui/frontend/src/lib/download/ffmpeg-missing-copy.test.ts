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

// The bodies themselves live in the per-locale namespace sources; pin
// the recovery guidance so a locale edit can't silently drop the
// yt-dlp alternative the preflight now honors.
import en from '../../../messages/en/download.json';
import es419 from '../../../messages/es-419/download.json';
import ptBR from '../../../messages/pt-BR/download.json';
import ru from '../../../messages/ru/download.json';

describe('missing-tool modal bodies', () => {
	const locales: Record<string, Record<string, string>> = {
		en,
		'es-419': es419,
		'pt-BR': ptBR,
		ru
	};
	const bodyKeys = [
		'error_ffmpeg_missing_body_win32',
		'error_ffmpeg_missing_body_linux',
		'error_ffmpeg_missing_body_darwin'
	];

	it('every locale body offers yt-dlp as an alternative', () => {
		for (const [locale, messages] of Object.entries(locales)) {
			for (const key of bodyKeys) {
				expect(messages[key], `${locale}.${key}`).toContain('yt-dlp');
			}
		}
	});

	it('every locale body conditions the auto-update claim on the setting', () => {
		// "updates automatically on launch" is false while the
		// auto-update toggle is off; the claim must point at the
		// House rules setting that controls it.
		const settingsRef: Record<string, string> = {
			en: 'House rules',
			'es-419': 'Reglas de la casa',
			'pt-BR': 'Regras da casa',
			ru: 'Правила дома'
		};
		for (const [locale, messages] of Object.entries(locales)) {
			for (const key of bodyKeys) {
				expect(messages[key], `${locale}.${key}`).toContain(settingsRef[locale]);
			}
		}
	});

	it('every locale body ties the yt-dlp alternative to an up-to-date script', () => {
		// The preflight only accepts yt-dlp when the active script's
		// download mode does (4.15+). A stale pre-4.15 cache requires
		// ffmpeg, so unqualified "yt-dlp also works" copy would tell
		// those users to retry an instruction they already satisfy.
		const qualifier: Record<string, string> = {
			en: 'up to date',
			'es-419': 'está actualizado',
			'pt-BR': 'estiver atualizado',
			ru: 'обновлён'
		};
		for (const [locale, messages] of Object.entries(locales)) {
			for (const key of bodyKeys) {
				expect(messages[key], `${locale}.${key}`).toContain(qualifier[locale]);
			}
		}
	});
});
