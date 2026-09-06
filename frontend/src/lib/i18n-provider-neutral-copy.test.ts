// The catalogue copy names no provider. Playback resolves against
// the providers in order, so "not in the catalogue" is whichever of
// them answered, and a string naming one of them would be wrong the
// moment the other answers.
//
// Swept at the message-source layer, like the retired-CLI sweep
// beside this file: the per-locale sources are the whole surface,
// and a locale added later is swept without editing this file.
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const MESSAGES = path.resolve(__dirname, '../../messages');
const LOCALES = fs
	.readdirSync(MESSAGES, { withFileTypes: true })
	.filter((e) => e.isDirectory())
	.map((e) => e.name)
	.sort();

/** The detail-page keys that describe the streaming catalogue. */
const CATALOGUE_KEYS = [
	'ep_disabled_tooltip',
	'ep_recheck_busy',
	'ep_recheck_idle',
	'ep_recheck_still_gated',
	'error_play_no_results',
	'unavailable_message'
];

describe('the catalogue copy names no provider', () => {
	it('the locale enumeration found the known locales', () => {
		expect(LOCALES).toEqual(expect.arrayContaining(['en', 'es-419', 'pt-BR', 'ru']));
	});

	for (const locale of LOCALES) {
		it(`${locale} catalogue strings name neither provider`, () => {
			const detail = JSON.parse(
				fs.readFileSync(path.join(MESSAGES, locale, 'detail.json'), 'utf8')
			) as Record<string, string>;
			const offenders: string[] = [];
			for (const key of CATALOGUE_KEYS) {
				const value = detail[key];
				expect(value, `${locale}/detail.json lacks ${key}`).toBeTypeOf('string');
				if (/anidb|hianime/i.test(value)) offenders.push(key);
			}
			expect(offenders, `strings naming a provider: ${offenders.join(', ')}`).toEqual([]);
		});
	}
});
