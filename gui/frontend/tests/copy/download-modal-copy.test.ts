// Copy-source pins for the missing-tool modal bodies. This file
// lives OUTSIDE src/ deliberately: it asserts on the i18n SOURCE
// files (messages/<locale>/<ns>.json), and gui/frontend/src/ may not
// import from outside its own tree (tests/arch/dep_direction.sh).
// Fixture-pins on repo data are not app source, so they sit here and
// vitest includes them explicitly.

import { describe, it, expect } from 'vitest';

// The bodies themselves live in the per-locale namespace sources; pin
// the recovery guidance so a locale edit can't silently drop the
// yt-dlp alternative the preflight now honors.
import en from '../../messages/en/download.json';
import es419 from '../../messages/es-419/download.json';
import ptBR from '../../messages/pt-BR/download.json';
import ru from '../../messages/ru/download.json';

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
