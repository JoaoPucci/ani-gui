// The diagnostics page describes the Home row styles. A banded row
// shows its band only while the pointer is over it or focus is inside
// it (Strip.svelte, `:hover` and `:focus-within`), so the description
// must not say the band marks out the row at all times: in every
// locale it names the pointer, drops the old "whole row" claim, and
// does not narrow focus to the keyboard's — a click inside the row
// gives focus too, and the band shows for it. It names the home page as
// the page's other descriptions do.

import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const ROOT = resolve(process.cwd(), 'messages');
const LOCALES = readdirSync(ROOT, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

/** Per locale: the word for the mouse pointer, the old copy's
 *  "whole row", and the word that would narrow focus to the keyboard. */
const WORDS: Record<
	string,
	{ pointer: string; wholeRow: string; keyboard: string; homePage: string }
> = {
	en: { pointer: 'pointer', wholeRow: 'whole row', keyboard: 'keyboard', homePage: 'home page' },
	'es-419': {
		pointer: 'puntero',
		wholeRow: 'fila entera',
		keyboard: 'teclado',
		homePage: 'página de inicio'
	},
	'pt-BR': {
		pointer: 'ponteiro',
		wholeRow: 'fileira inteira',
		keyboard: 'teclado',
		homePage: 'página inicial'
	},
	ru: {
		pointer: 'указатель',
		wholeRow: 'весь ряд',
		keyboard: 'клавиатур',
		homePage: 'главной странице'
	}
};

function description(locale: string): string {
	const d = JSON.parse(readFileSync(join(ROOT, locale, 'diagnostics.json'), 'utf8'));
	return String(d.row_style_description).toLocaleLowerCase(locale);
}

describe('the row style description', () => {
	it('covers every shipped locale', () => {
		expect(LOCALES.sort()).toEqual(Object.keys(WORDS).sort());
	});

	for (const locale of LOCALES) {
		it(`says when the band shows in ${locale}`, () => {
			const text = description(locale);
			const words = WORDS[locale];
			expect(text).toContain(words.pointer);
			expect(text).not.toContain(words.wholeRow);
			expect(text).not.toContain(words.keyboard);
		});

		it(`names the home page as the banner framing description does in ${locale}`, () => {
			// One phrase per term: the page's other descriptions say "the
			// home page", not "Home".
			expect(description(locale)).toContain(WORDS[locale].homePage);
		});
	}
});
