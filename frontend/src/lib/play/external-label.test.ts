// The play page's "Open in …" item and the Settings hint that names
// it. The item says which player it opens, from the configured
// player flavour, so a user who picked VLC reads "Open in VLC". A
// Custom flavour has no product name, so it says "external player".
import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';

import { externalOpenLabel } from './external-label';
import { m } from '$lib/paraglide/messages';
import type { ExternalPlayerKind } from '$lib/api';

type Locale = 'en' | 'pt-BR' | 'es-419' | 'ru';

const EXPECTED: Record<Locale, Record<ExternalPlayerKind, string>> = {
	en: {
		mpv: 'Open in mpv',
		vlc: 'Open in VLC',
		iina: 'Open in IINA',
		custom: 'Open in external player'
	},
	'pt-BR': {
		mpv: 'Abrir no mpv',
		vlc: 'Abrir no VLC',
		iina: 'Abrir no IINA',
		custom: 'Abrir no player externo'
	},
	'es-419': {
		mpv: 'Abrir en mpv',
		vlc: 'Abrir en VLC',
		iina: 'Abrir en IINA',
		custom: 'Abrir en el reproductor externo'
	},
	ru: {
		mpv: 'Открыть в mpv',
		vlc: 'Открыть в VLC',
		iina: 'Открыть в IINA',
		custom: 'Открыть во внешнем плеере'
	}
};

const KINDS: ExternalPlayerKind[] = ['mpv', 'vlc', 'iina', 'custom'];
const LOCALES = Object.keys(EXPECTED) as Locale[];

describe('externalOpenLabel', () => {
	it.each(LOCALES.flatMap((l) => KINDS.map((k) => [l, k] as const)))(
		'in %s names the %s player',
		(locale, kind) => {
			expect(externalOpenLabel(kind, locale)).toBe(EXPECTED[locale][kind]);
		}
	);

	it('names a brand player by its own name in every locale', () => {
		fc.assert(
			fc.property(
				fc.constantFrom(...LOCALES),
				fc.constantFrom<ExternalPlayerKind>('mpv', 'vlc', 'iina'),
				(locale, kind) => {
					const brand = { mpv: 'mpv', vlc: 'VLC', iina: 'IINA' }[kind as 'mpv' | 'vlc' | 'iina'];
					expect(externalOpenLabel(kind, locale)).toContain(brand);
				}
			)
		);
	});

	it('gives every flavour a different label in every locale', () => {
		for (const locale of LOCALES) {
			const labels = KINDS.map((k) => externalOpenLabel(k, locale));
			expect(new Set(labels).size).toBe(KINDS.length);
		}
	});
});

describe('the Settings hint names the same button', () => {
	it.each(LOCALES.flatMap((l) => KINDS.map((k) => [l, k] as const)))(
		'in %s, for %s, quotes the button the play page shows',
		(locale, kind) => {
			const hint = m.settings_field_external_player_hint_prefix(
				{ button: externalOpenLabel(kind, locale) },
				{ locale }
			);
			expect(hint).toContain(EXPECTED[locale][kind]);
		}
	);
});
