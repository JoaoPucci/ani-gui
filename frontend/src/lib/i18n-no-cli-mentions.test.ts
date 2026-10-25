// The app does not narrate its origins. It began as a desktop shell
// over a shell scraper, and every trace of that belongs to history —
// the repository's provenance notes, not the interface. A settings
// row that names the retired script and then explains it is gone is
// a ledger entry, and this is an app, not a ledger.
//
// Swept at the message-source layer: the localized copy, the words
// the app authors. That is the deliberate scope. Runtime data can
// still carry the old name — the boot sweep reports the paths it
// deleted verbatim, and on one launch per upgraded install one of
// those paths ends in the script's filename — and that is a fact
// about the user's disk, not the app narrating. A green run says
// nothing about rendered data, only about the copy.
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const MESSAGES = path.resolve(__dirname, '../../messages');
// Derived, so a locale added later is swept without anyone editing
// this file. The compiled per-locale bundles beside these directories
// are generated from them and gitignored, so the sources are the
// whole surface.
const LOCALES = fs
	.readdirSync(MESSAGES, { withFileTypes: true })
	.filter((e) => e.isDirectory())
	.map((e) => e.name)
	.sort();

// Every bundle is read while the file is collected, not inside the
// test that sweeps it: a test's 5 s budget is wall time, and cold reads
// queued behind another build's disk writes can spend it before a
// single string is checked.
const BUNDLES = new Map(
	LOCALES.map((locale) => {
		const dir = path.join(MESSAGES, locale);
		const files = fs
			.readdirSync(dir)
			.filter((file) => file.endsWith('.json'))
			.map((file) => ({ file, data: JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8')) }));
		return [locale, files];
	})
);

describe('no user-visible string names the retired CLI', () => {
	it('the locale enumeration found the known locales', () => {
		// An empty or partial listing must not read as a clean sweep.
		expect(LOCALES).toEqual(expect.arrayContaining(['en', 'es-419', 'pt-BR', 'ru']));
	});

	for (const locale of LOCALES) {
		it(`${locale} bundles carry no ani-cli or pystardust mention`, () => {
			const offenders: string[] = [];
			for (const { file, data } of BUNDLES.get(locale)!) {
				for (const [key, value] of Object.entries(data)) {
					// A message with plural variants is an array of objects, not
					// a string; serialising it sweeps every variant's text.
					if (/ani-cli|pystardust/i.test(JSON.stringify(value))) offenders.push(`${file}:${key}`);
				}
			}
			expect(offenders, `strings naming the retired script: ${offenders.join(', ')}`).toEqual([]);
		});
	}
});
