// When disconnecting cannot remove the stored sign-in, the app says so.
// The message used to send the user to "ani-gui logs": there is no log
// file to read (docs/development.md), and a build started from the
// desktop has no terminal. It also said "local token", and three
// locales carried the English sentence. These cases hold the message,
// and the account bundle around it, to what a user can act on.

import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const ROOT = resolve(process.cwd(), 'messages');
const LOCALES = readdirSync(ROOT, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

function account(locale: string): Record<string, string> {
	return JSON.parse(readFileSync(join(ROOT, locale, 'account.json'), 'utf8'));
}

const KEY = 'disconnect_error_token_clear';

describe('the message when a sign-in cannot be removed', () => {
	it('covers every shipped locale', () => {
		expect(LOCALES.sort()).toEqual(['en', 'es-419', 'pt-BR', 'ru']);
	});

	for (const locale of LOCALES) {
		it(`sends nobody to logs, and uses no semicolon, in ${locale}`, () => {
			const text = account(locale)[KEY];
			expect(text).toBeTruthy();
			expect(text).not.toMatch(/\blogs?\b/i);
			expect(text).not.toMatch(/token/i);
			expect(text).not.toContain(';');
		});
	}

	for (const locale of LOCALES.filter((l) => l !== 'en')) {
		it(`is translated in ${locale}`, () => {
			expect(account(locale)[KEY]).not.toBe(account('en')[KEY]);
		});
	}

	it('no account message in any locale points at logs', () => {
		for (const locale of LOCALES) {
			for (const [key, text] of Object.entries(account(locale))) {
				expect(String(text), `${locale} ${key}`).not.toMatch(/\blogs?\b/i);
			}
		}
	});
});
