// The About page's copy is read as short sentences, with no semicolon,
// in every locale. A semicolon in UI text reads as machine-written,
// especially in pt-BR, and the About page's tool notes had chained
// their clauses with them.

import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const ROOT = resolve(process.cwd(), 'messages');
const LOCALES = readdirSync(ROOT, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

describe('the About page copy', () => {
	it('covers every shipped locale', () => {
		expect(LOCALES.sort()).toEqual(['en', 'es-419', 'pt-BR', 'ru']);
	});

	for (const locale of LOCALES) {
		it(`has no semicolon in ${locale}`, () => {
			const about = JSON.parse(readFileSync(join(ROOT, locale, 'about.json'), 'utf8'));
			const withSemicolon = Object.entries(about)
				.filter(([, text]) => String(text).includes(';'))
				.map(([key]) => key);
			expect(withSemicolon).toEqual([]);
		});
	}
});
