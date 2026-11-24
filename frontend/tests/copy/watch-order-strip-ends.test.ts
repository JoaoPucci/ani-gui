// Pins the lines an empty strip column shows, in every locale, as the
// maintainer worded them. The column before the first title says the
// story starts here. The column after the last one says nothing comes
// after it yet, and a side story's end says the same with the same
// message, so the two cannot drift apart.
//
// Reads the namespace sources as data, like the other copy pins here.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, it, expect } from 'vitest';

const WORDING = {
	en: { first: 'The story starts here', last: 'Nothing after this yet' },
	'pt-BR': { first: 'A história começa aqui', last: 'Ainda não tem nada depois deste' },
	'es-419': { first: 'La historia empieza aquí', last: 'Todavía no hay nada después' },
	ru: { first: 'Отсюда всё начинается', last: 'Дальше пока ничего нет' }
} as const;

function catalogue(locale: string): Record<string, unknown> {
	const file = fileURLToPath(new URL(`../../messages/${locale}/watchorder.json`, import.meta.url));
	return JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
}

describe("the strip's empty columns", () => {
	for (const [locale, words] of Object.entries(WORDING)) {
		it(`reads as worded in ${locale}`, () => {
			const messages = catalogue(locale);
			expect(messages.strip_first).toBe(words.first);
			expect(messages.strip_last).toBe(words.last);
		});

		it(`has one message for the end of the story and of a side story in ${locale}`, () => {
			expect(catalogue(locale)).not.toHaveProperty('strip_side_end');
		});
	}
});
