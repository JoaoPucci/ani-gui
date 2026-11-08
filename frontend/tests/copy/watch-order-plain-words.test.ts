// Pins the watch order's copy to the viewer's words. "Run", "entry",
// "loop", "fork" and "join" name how the ordering works inside the
// app, and viewers did not understand them on screen, so no message a
// viewer sees or hears may use them, in any locale. Portuguese also
// keeps "ligado" out: next to "Por vínculo" it reads as "switched on".
//
// Reads the namespace sources as data, like the other copy pins here.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, it, expect } from 'vitest';

const LOCALES = ['en', 'pt-BR', 'es-419', 'ru'] as const;

const BANNED: Record<(typeof LOCALES)[number], RegExp> = {
	en: /\b(run|runs|entry|entries|loop|loops|fork|forks|join|joins)\b/i,
	'pt-BR': /\b(sequência|ciclo|entrada|ligad[oa]s?)\b/i,
	'es-419': /\b(secuencia|ciclo|entrada)\b/i,
	ru: /(цепоч|круг|запис)/i
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
