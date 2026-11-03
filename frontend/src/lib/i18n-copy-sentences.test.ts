// The app's text is written as sentences a person would write: each
// says one thing and stops, then the next begins. A semicolon joining
// two of them is the mark of text that was generated or translated
// word for word, and it reads worst in Brazilian Portuguese, so no
// message in any locale carries one.
//
// The rule is the character, not a reading of the sentence: no
// message needs a semicolon for anything else, and a check that tried
// to tell a joining semicolon from some other kind would be guessing.
//
// The main process's dialogs (electron/lib/main-messages.cjs) hold to
// the same rule in their own test. The launch splash's status lines,
// proposed in open PR #357 for that same table, are exempt from the
// copy sweep's em-dash rewrites: their dash is the maintainer's
// choice. Only semicolons are checked, so they need no exclusion.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(__dirname, '..', '..', 'messages');
const locales = readdirSync(root, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name)
	.sort();

/** Every string in a message tree: plain messages and plural arms. */
function strings(value: unknown): string[] {
	if (typeof value === 'string') return [value];
	if (Array.isArray(value)) return value.flatMap(strings);
	if (value && typeof value === 'object') return Object.values(value).flatMap(strings);
	return [];
}

describe("the app's text", () => {
	it('is read for every packaged locale', () => {
		expect(locales).toEqual(['en', 'es-419', 'pt-BR', 'ru']);
	});

	for (const locale of locales) {
		const files = readdirSync(join(root, locale))
			.filter((f) => f.endsWith('.json'))
			.sort();

		it(`has message files in ${locale}`, () => {
			expect(files.length).toBeGreaterThan(0);
		});

		for (const file of files) {
			it(`joins no two sentences with a semicolon in ${locale}/${file}`, () => {
				const bundle = JSON.parse(readFileSync(join(root, locale, file), 'utf8'));
				const joined = Object.entries(bundle)
					.filter(([key]) => key !== '$schema')
					.flatMap(([key, value]) => strings(value).map((s) => [key, s] as const))
					.filter(([, s]) => s.includes(';'));
				expect(joined).toEqual([]);
			});
		}
	}
});
