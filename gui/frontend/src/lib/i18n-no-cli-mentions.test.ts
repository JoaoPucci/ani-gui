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

describe('no user-visible string names the retired CLI', () => {
	it('the locale enumeration found the known locales', () => {
		// An empty or partial listing must not read as a clean sweep.
		expect(LOCALES).toEqual(expect.arrayContaining(['en', 'es-419', 'pt-BR', 'ru']));
	});

	for (const locale of LOCALES) {
		it(`${locale} bundles carry no ani-cli or pystardust mention`, () => {
			const dir = path.join(MESSAGES, locale);
			const offenders: string[] = [];
			for (const file of fs.readdirSync(dir)) {
				if (!file.endsWith('.json')) continue;
				const data = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
				for (const [key, value] of Object.entries(data)) {
					if (typeof value !== 'string') continue;
					if (/ani-cli|pystardust/i.test(value)) offenders.push(`${file}:${key}`);
				}
			}
			expect(offenders, `strings naming the retired script: ${offenders.join(', ')}`).toEqual([]);
		});
	}
});
