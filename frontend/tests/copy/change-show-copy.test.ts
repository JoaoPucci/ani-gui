// The words of "Is this the show you meant?", in every locale.
//
// When a Continue card's show was only a guess, the app asks whether
// it is the show the user meant, and if not, which show it is. How the
// app keeps the answer (a link from the history line to a Kitsu show,
// made because the show was guessed) is the app's concern, not the
// user's. So none of the messages of that flow, on any surface (the
// player's slip and ⋯ entry, the picker, Home's pill and failed resume,
// the show page's line), speaks of linking, of a guess, or of the
// Continue row as a thing: the user is asked about a show and told
// which show plays. And like the rest of the app's text, none joins
// two sentences with a semicolon.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(__dirname, '..', '..', 'messages');

/** The flow's messages: each bundle's keys of it. */
const flow: Record<string, (key: string) => boolean> = {
	'detail.json': (k) => k.startsWith('link_'),
	'home.json': (k) => k.startsWith('link_') || k === 'resume_guess_note',
	'play.json': (k) => k.startsWith('link_')
};

/** What each locale's words for the app's side of it look like. */
const unsaid: Record<string, RegExp[]> = {
	en: [/link/i, /guess/i, /continue watching/i, /continue card/i],
	'pt-BR': [/vincul/i, /v[íi]nculo/i, /deduz/i, /continuar assistindo/i],
	'es-419': [/vincul/i, /v[íi]nculo/i, /deduc/i, /dedujo/i],
	ru: [/связ/i, /угада/i, /продолжить просмотр/i]
};

function messages(locale: string): [string, string][] {
	return Object.entries(flow).flatMap(([file, ours]) => {
		const bundle = JSON.parse(readFileSync(join(root, locale, file), 'utf8')) as Record<
			string,
			unknown
		>;
		return Object.entries(bundle)
			.filter(([k, v]) => ours(k) && typeof v === 'string')
			.map(([k, v]) => [`${file}:${k}`, v as string] as [string, string]);
	});
}

describe('the show question', () => {
	for (const [locale, words] of Object.entries(unsaid)) {
		it(`asks about a show, never about a link or a guess, in ${locale}`, () => {
			const said = messages(locale).filter(([, text]) => words.some((w) => w.test(text)));
			expect(said).toEqual([]);
		});

		it(`joins no two sentences with a semicolon in ${locale}`, () => {
			expect(messages(locale).filter(([, text]) => text.includes(';'))).toEqual([]);
		});
	}

	it('has every message of it in every locale', () => {
		const keys = (l: string) => messages(l).map(([k]) => k);
		for (const locale of Object.keys(unsaid))
			expect(keys(locale).sort()).toEqual(keys('en').sort());
	});
});
