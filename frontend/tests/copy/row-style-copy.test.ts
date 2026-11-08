// The diagnostics page describes the Home row styles. A banded row
// shows its band only while the pointer is over it or it has keyboard
// focus (Strip.svelte), so the description must not say the band
// marks out the row at all times: in every locale it names the
// pointer, the condition the band appears under.

import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const ROOT = resolve(process.cwd(), 'messages');
const LOCALES = readdirSync(ROOT, { withFileTypes: true })
	.filter((d) => d.isDirectory())
	.map((d) => d.name);

/** The word for the mouse pointer in each locale's copy. */
const POINTER: Record<string, string> = {
	en: 'pointer',
	'es-419': 'puntero',
	'pt-BR': 'ponteiro',
	ru: 'указатель'
};

function description(locale: string): string {
	const d = JSON.parse(readFileSync(join(ROOT, locale, 'diagnostics.json'), 'utf8'));
	return String(d.row_style_description).toLocaleLowerCase(locale);
}

describe('the row style description', () => {
	it('covers every shipped locale', () => {
		expect(LOCALES.sort()).toEqual(Object.keys(POINTER).sort());
	});

	for (const locale of LOCALES) {
		it(`says when the band shows in ${locale}`, () => {
			expect(description(locale)).toContain(POINTER[locale]);
		});
	}
});
