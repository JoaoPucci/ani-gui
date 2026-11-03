// The download copy is read out now: a download's state and reports
// describe the dock's menu items, and a screen reader speaks them. No
// string joins two sentences with a semicolon, in any locale; each says
// one thing and stops, then the next.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(__dirname, '..', '..', '..', 'messages');
const locales = readdirSync(root, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

/** Every string in a message tree: plain messages and plural arms. */
function strings(value: unknown): string[] {
	if (typeof value === 'string') return [value];
	if (Array.isArray(value)) return value.flatMap(strings);
	if (value && typeof value === 'object') return Object.values(value).flatMap(strings);
	return [];
}

describe('the download copy', () => {
	it('is read for every packaged locale', () => {
		expect(locales.sort()).toEqual(['en', 'es-419', 'pt-BR', 'ru']);
	});

	for (const locale of locales) {
		it(`joins no two sentences with a semicolon in ${locale}`, () => {
			const bundle = JSON.parse(readFileSync(join(root, locale, 'download.json'), 'utf8'));
			const joined = Object.entries(bundle)
				.filter(([key]) => key !== '$schema')
				.flatMap(([key, value]) => strings(value).map((s) => [key, s] as const))
				.filter(([, s]) => s.includes(';'));
			expect(joined).toEqual([]);
		});
	}
});
