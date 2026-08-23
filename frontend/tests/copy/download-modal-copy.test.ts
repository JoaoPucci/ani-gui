// Copy-source pins for the missing-tool modal bodies. This file
// lives OUTSIDE src/ deliberately: it asserts on the i18n SOURCE
// files (messages/<locale>/<ns>.json), and frontend/src/ may not
// import from outside its own tree (tests/arch/dep_direction.sh).
// Fixture-pins on repo data are not app source, so they sit here and
// vitest includes them explicitly.

import { describe, it, expect } from 'vitest';

// The bodies themselves live in the per-locale namespace sources; pin
// the recovery guidance so a locale edit can't reintroduce advice we
// deliberately removed.
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

	it('no locale body recommends yt-dlp', () => {
		// REVERSED, with justification per AGENTS.md. These bodies used
		// to advise installing yt-dlp instead of ffmpeg. Measured
		// against a real TS-segment stream with ffmpeg off PATH, that
		// configuration exits 0 and writes raw MPEG-TS carrying an .mp4
		// name — the download reports success and the file is not what
		// it claims. Steering users into it is worse than the modal
		// they already have, so the sentence is gone and stays gone.
		//
		// The preflight still ACCEPTS yt-dlp, matching what the 4.15
		// script itself requires; the spawn now fails loudly when the
		// repackaging step was skipped. Recommending it is the part
		// that was wrong, not permitting it.
		for (const [locale, messages] of Object.entries(locales)) {
			for (const key of bodyKeys) {
				expect(messages[key], `${locale}.${key}`).not.toContain('yt-dlp');
			}
		}
	});

	// The auto-update-setting pin went with the sentence it qualified.
	// It asserted every body names the House rules setting, but that
	// reference existed only inside the yt-dlp advice — there is no
	// longer a claim about script updates for it to condition.
});
