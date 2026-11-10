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
// the same rules in their own test. The launch splash's status lines,
// in that same table, keep their em dash: it is the maintainer's
// choice.
//
// An em dash set between spaces is the same tell as the semicolon: an
// aside bolted onto a sentence that a full stop or a comma would have
// ended. The messages that keep one are listed below by name, so a new
// dash is a decision someone makes, not a habit that slips in.
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

/**
 * Messages allowed an em dash between spaces, and why.
 *
 * - A page or link title, "ani-gui — home": the dash separates the
 *   app's name from the page, as a window title does.
 * - The diagnostics page, a tool for the developer, whose link hints
 *   read as "link — what it opens" and whose history rows as "title —
 *   episode".
 * - Russian, where the dash stands for the verb "is" that the language
 *   leaves out («Sub — японская озвучка»): there it is grammar, not an
 *   aside.
 */
const DASH_ALLOWED: Record<string, string[]> = {
	'*': [
		'app.home_link_title',
		'diagnostics.title',
		'diagnostics.open_search_link_hint',
		'diagnostics.open_test_stream_link_hint',
		'diagnostics.history_entry_format'
	],
	ru: ['detail.episodes_source_credit', 'settings.field_audio_hint']
};

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

			it(`sets no aside off with an em dash in ${locale}/${file}`, () => {
				const ns = file.slice(0, -'.json'.length);
				const allowed = new Set([...DASH_ALLOWED['*'], ...(DASH_ALLOWED[locale] ?? [])]);
				const bundle = JSON.parse(readFileSync(join(root, locale, file), 'utf8'));
				const dashed = Object.entries(bundle)
					.filter(([key]) => key !== '$schema' && !allowed.has(`${ns}.${key}`))
					.flatMap(([key, value]) => strings(value).map((s) => [key, s] as const))
					.filter(([, s]) => s.includes(' — '));
				expect(dashed).toEqual([]);
			});
		}
	}
});
