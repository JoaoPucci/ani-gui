// The app does not narrate its origins. It began as a desktop shell
// over a shell scraper, and every trace of that belongs to history —
// the repository's provenance notes, not the interface. A settings
// row that names the retired script and then explains it is gone is
// a ledger entry, and this is an app, not a ledger.
//
// Swept at the message-source layer, where every user-visible string
// lives, so the rule covers all locales and any surface at once.
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const MESSAGES = path.resolve(__dirname, '../../messages');
const LOCALES = ['en', 'es-419', 'pt-BR', 'ru'];

describe('no user-visible string names the retired CLI', () => {
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
