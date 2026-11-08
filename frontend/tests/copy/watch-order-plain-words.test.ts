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

const BANNED: Record<(typeof LOCALES)[number], RegExp> = {
	en: /\b(runs?|entry|entries|loops?|forks?|joins?)\b/i,
	'pt-BR': /\b(sequências?|ciclos?|entradas?|ligad[oa]s?|bifurca\w*|ramifica\w*|junç(ão|ões))\b/iu,
	'es-419': /\b(secuencias?|ciclos?|bucles?|entradas?|bifurca\w*|ramifica\w*|unión|uniones)\b/iu,
	// Stems, since \b knows only ASCII letters; "круг" is anchored so
	// "вокруг" (around) is not caught.
	ru: /(цепоч|(^|[^а-яё])круг|цикл|петл|развилк|запис)/iu
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
