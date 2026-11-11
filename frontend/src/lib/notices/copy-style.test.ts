// The notification center's words read as short native sentences: no
// semicolon joins two clauses, in any locale. A semicolon in UI copy
// reads as machine-written, Brazilian Portuguese most of all.
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const MESSAGES = path.resolve(__dirname, '../../../messages');

/** Every string in a message file, plural variants included. */
function strings(v: unknown): string[] {
	if (typeof v === 'string') return [v];
	if (Array.isArray(v)) return v.flatMap(strings);
	if (v && typeof v === 'object') return Object.values(v).flatMap(strings);
	return [];
}

describe('the notices copy', () => {
	for (const locale of ['en', 'pt-BR', 'es-419', 'ru']) {
		it(`has no semicolons in ${locale}`, () => {
			const file = JSON.parse(
				fs.readFileSync(path.join(MESSAGES, locale, 'notices.json'), 'utf8')
			) as Record<string, unknown>;
			const found = Object.entries(file).flatMap(([key, v]) =>
				strings(v)
					.filter((s) => s.includes(';'))
					.map((s) => `${key}: ${s}`)
			);
			expect(found).toEqual([]);
		});
	}
});

describe("the published notices' and the donation ask's words", () => {
	const KEYS = [
		'published_meta',
		'published_link_label',
		'published_link_action',
		'donation_ask',
		'donation_donate',
		'donation_later'
	];
	const read = (locale: string) =>
		JSON.parse(fs.readFileSync(path.join(MESSAGES, locale, 'notices.json'), 'utf8')) as Record<
			string,
			unknown
		>;

	for (const locale of ['pt-BR', 'es-419', 'ru']) {
		it(`are written for ${locale}, not copied from English`, () => {
			const en = read('en');
			const own = read(locale);
			for (const key of KEYS) {
				expect(typeof own[key], key).toBe('string');
				if (key !== 'published_link_label' || locale !== 'pt-BR') {
					expect(own[key], key).not.toBe(en[key]);
				}
			}
		});
	}
});
