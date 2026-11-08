// Pins the watch order's copy to the viewer's words. "Run", "entry",
// "loop", "fork" and "join" name how the ordering works inside the
// app, and viewers did not understand them on screen, so no message a
// viewer sees or hears may use them, in any locale. Portuguese also
// keeps "ligado" out: next to "Por vínculo" it reads as "switched on".
//
// The lists name the words known to have been used, with their
// plurals; they are not a full vocabulary check, so a new synonym of
// one of these terms gets past it.
//
// Reads the namespace sources as data, like the other copy pins here.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, it, expect } from 'vitest';

const LOCALES = ['en', 'pt-BR', 'es-419', 'ru'] as const;

// A term stands alone: no letter of any script touches either side of
// it, which `\b` cannot say for accented or Cyrillic letters.
const word = (terms: string) => new RegExp(`(?<!\\p{L})(${terms})(?!\\p{L})`, 'iu');
// A stem starts a word and may run on: "цикл" catches "циклы" but not
// "энциклопедия".
const stem = (stems: string) => new RegExp(`(?<!\\p{L})(${stems})`, 'iu');

const BANNED: Record<(typeof LOCALES)[number], RegExp> = {
	en: word('runs?|entry|entries|loops?|forks?|joins?'),
	'pt-BR': word(
		'sequências?|ciclos?|entradas?|ligad[oa]s?|bifurca\\p{L}*|ramifica\\p{L}*|junç(ão|ões)'
	),
	'es-419': word(
		'secuencias?|ciclos?|bucles?|entradas?|bifurca\\p{L}*|ramifica\\p{L}*|unión|uniones'
	),
	ru: stem('цепоч|круг|цикл|петл|развилк|запис')
};

/** Every text a message can show: plain strings and each plural variant. */
function texts(value: unknown): string[] {
	if (typeof value === 'string') return [value];
	if (Array.isArray(value))
		return value.flatMap((v) => Object.values((v as { match: Record<string, string> }).match));
	return [];
}

const catalogues = LOCALES.map((locale) => {
	const file = fileURLToPath(new URL(`../../messages/${locale}/watchorder.json`, import.meta.url));
	return { locale, messages: JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown> };
});

describe('the watch order speaks the viewer’s words', () => {
	it.each(catalogues)('$locale', ({ locale, messages }) => {
		const found = Object.entries(messages).flatMap(([key, value]) =>
			texts(value)
				.filter((t) => BANNED[locale].test(t))
				.map((t) => `${key}: ${t}`)
		);
		expect(found).toEqual([]);
	});
});

describe('the Up next key over an entry not aired yet', () => {
	// It says what comes next for the viewer, once it is out, as the
	// other locales do. Russian once said "will be released next", which
	// is about release order and agrees with a noun it never names.
	it('reads "Next, when it airs" in Russian too', () => {
		const ru = catalogues.find((c) => c.locale === 'ru')!.messages;
		expect(ru.upnext_coming).toBe('Далее, когда выйдет');
	});
});
