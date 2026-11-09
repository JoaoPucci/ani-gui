// The Watch Later empty row tells the user to press Refresh. In every
// locale that hint has to use the verb the button carries: es-419 and
// pt-BR labelled the button Recargar / Recarregar while the hint still
// said Actualiza / Atualize, so it pointed at a button that is not
// there. The check is on the verb's stem — the action's label less its
// last two letters — so a conjugated form ("Refresh" → "Refresh",
// "Recargar" → "Recarga", "Обновить" → "Обновите") passes.

import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const ROOT = resolve(process.cwd(), 'messages');
const LOCALES = readdirSync(ROOT, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

function account(locale: string): Record<string, unknown> {
	return JSON.parse(readFileSync(join(ROOT, locale, 'account.json'), 'utf8'));
}

describe('the Watch Later hint', () => {
	it('covers every shipped locale', () => {
		expect(LOCALES.sort()).toEqual(['en', 'es-419', 'pt-BR', 'ru']);
	});

	for (const locale of LOCALES) {
		it(`names the Refresh action by its own verb in ${locale}`, () => {
			const a = account(locale);
			const action = String(a.watch_later_refresh_action);
			const stem = action.slice(0, -2).toLocaleLowerCase(locale);
			expect(stem.length).toBeGreaterThan(3);
			expect(String(a.watch_later_empty_hint).toLocaleLowerCase(locale)).toContain(stem);
		});
	}
});
