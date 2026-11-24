// The franchise page's fork lists its ways side by side under one
// "N ways to continue" heading. The maintainer dropped the label each
// column carried ("Way 1 of 2"), so no catalogue keeps a message for it.
//
// Reads the namespace sources as data, like the other copy pins here.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, it, expect } from 'vitest';

const LOCALES = ['en', 'pt-BR', 'es-419', 'ru'] as const;

function catalogue(locale: string): Record<string, unknown> {
	const file = fileURLToPath(new URL(`../../messages/${locale}/watchorder.json`, import.meta.url));
	return JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
}

describe("the fork's columns", () => {
	for (const locale of LOCALES) {
		it(`carry no per-column label in ${locale}, and keep the heading`, () => {
			const messages = catalogue(locale);
			expect(messages).not.toHaveProperty('strip_way');
			expect(messages).toHaveProperty('strip_ways');
		});
	}
});
