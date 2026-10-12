// `describeError` chooses a failure's sentence by the backend's stable
// key, and by the envelope's `kind` only when the key is absent or
// unknown — except where a field says more than the key does (an
// upstream's 404 or 5xx, a rate limit's wait). The key list is the
// backend's own (`backend/error-keys.json`, held to the variants by
// `cargo test`), and the catalogues under
// `frontend/messages/<locale>/backend.json` are read here as files so
// the test states what each locale shows for each key — not what some
// other module says it does.

import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import * as fc from 'fast-check';

import { describeError } from './describe-error';
import { getLocale, locales, overwriteGetLocale } from '$lib/paraglide/runtime';

const KEYS: string[] = JSON.parse(
	readFileSync(new URL('../../../../backend/error-keys.json', import.meta.url), 'utf-8')
);

/** The per-key catalogue of one locale, as authored. */
function catalogue(locale: string): Record<string, string> {
	return JSON.parse(
		readFileSync(new URL(`../../../messages/${locale}/backend.json`, import.meta.url), 'utf-8')
	);
}

/** The message name a key compiles to: `error.a.b` → `error_a_b`. */
const nameOf = (key: string) => key.replaceAll('.', '_');

/** The structured fields a key's message interpolates. A key absent
 *  here takes none. The upstream status is one no field reading
 *  claims (404, 410, 429 and 5xx each have a sentence of their own),
 *  so the key's message is what shows. */
const FIELDS: Readonly<Record<string, Record<string, string | number>>> = {
	'error.network.upstream': { status: 400 },
	'error.player.spawn_failed': { binary: 'mpv' },
	'error.syncplay.spawn_failed': { binary: 'syncplay' }
};

/** The catalogue text with its placeholders filled the way the
 *  message would fill them. */
function rendered(text: string, fields: Record<string, string | number>): string {
	return text.replace(/\{(\w+)\}/g, (_, name: string) => String(fields[name]));
}

// The unit tier runs in node, where the runtime's localStorage
// strategy has nothing to read and `setLocale` cannot take effect, so
// the per-locale case overrides the lookup itself and restores it.
const originalGetLocale = getLocale;
afterEach(() => overwriteGetLocale(originalGetLocale));

describe('describeError resolves the key before the kind', () => {
	it('shows the key’s own sentence, not the kind’s grouping', () => {
		const keyed = describeError({ kind: 'metadata', key: 'error.metadata.source' });
		expect(keyed).toBe(catalogue('en').error_metadata_source);
		expect(keyed).not.toBe(describeError({ kind: 'metadata' }));
	});

	it('every published key has every locale’s own message', () => {
		const english = catalogue('en');
		for (const locale of locales) {
			overwriteGetLocale(() => locale);
			const texts = catalogue(locale);
			const generic = describeError({ kind: 'something_new' });
			for (const key of KEYS) {
				const fields = FIELDS[key] ?? {};
				const shown = describeError({ kind: 'something_new', key, ...fields });
				expect(shown, `${locale} ${key}`).toBe(rendered(texts[nameOf(key)], fields));
				expect(shown, `${locale} ${key}`).not.toBe(generic);
				expect(shown, `${locale} ${key}`).not.toContain(key);
				if (locale !== 'en') {
					// The compile hands a locale the English text for a
					// message it lacks; an untranslated entry would pass
					// the equality above and read as English on screen.
					expect(shown, `${locale} ${key} is untranslated`).not.toBe(
						rendered(english[nameOf(key)], fields)
					);
				}
			}
		}
	});

	it('fills a message’s placeholders from the envelope’s fields', () => {
		expect(
			describeError({ kind: 'upstream', key: 'error.network.upstream', status: 400 })
		).toContain('400');
		expect(
			describeError({
				kind: 'player_spawn_failed',
				key: 'error.player.spawn_failed',
				binary: 'vlc'
			})
		).toContain('vlc');
	});

	it('keeps the readings of a field that say more than the key does', () => {
		// An upstream status of 404/410, 429 or 5xx, and a rate limit's
		// advertised wait, each name the cause more precisely than the
		// key's one sentence for every status or every wait.
		const upstream = (status: number) =>
			describeError({ kind: 'upstream', key: 'error.network.upstream', status });
		for (const status of [404, 410, 429, 503]) {
			expect(upstream(status), String(status)).toBe(describeError({ kind: 'upstream', status }));
			expect(upstream(status), String(status)).not.toContain(String(status));
		}
		const waited = describeError({
			kind: 'rate_limited',
			key: 'error.network.rate_limited',
			retry_after_secs: 7
		});
		expect(waited).toContain('7');
		expect(waited).toBe(describeError({ kind: 'rate_limited', retry_after_secs: 7 }));
	});

	it('falls back to the kind when a message’s field is missing or empty', () => {
		// Rendering "undefined" or an empty name is worse than the
		// kind's coarser sentence.
		expect(describeError({ kind: 'upstream', key: 'error.network.upstream' })).toBe(
			describeError({ kind: 'upstream' })
		);
		expect(describeError({ kind: 'upstream', key: 'error.network.upstream', status: '503' })).toBe(
			describeError({ kind: 'upstream' })
		);
		expect(
			describeError({ kind: 'player_spawn_failed', key: 'error.player.spawn_failed', binary: '' })
		).toBe(describeError({ kind: 'player_spawn_failed' }));
	});

	it('falls back to the kind for a key it does not know, or that is not a string', () => {
		const byKind = describeError({ kind: 'network' });
		expect(describeError({ kind: 'network', key: 'error.network.something_new' })).toBe(byKind);
		expect(describeError({ kind: 'network', key: 7 })).toBe(byKind);
		expect(describeError({ kind: 'network', key: null })).toBe(byKind);
		// A key that is not a backend key must not reach a message by
		// its spelling alone.
		expect(describeError({ kind: 'network', key: 'errors.reason.timeout' })).toBe(byKind);
	});

	it('is generic when neither the key nor the kind is known', () => {
		expect(describeError({ kind: 'something_new', key: 'error.something.new' })).toBe(
			describeError({ kind: 'something_new' })
		);
	});

	it('the detail never changes the copy, whatever the key', () => {
		fc.assert(
			fc.property(fc.constantFrom(...KEYS), fc.string(), (key, detail) => {
				const fields = FIELDS[key] ?? {};
				expect(describeError({ kind: 'x', key, detail, ...fields })).toBe(
					describeError({ kind: 'x', key, ...fields })
				);
			})
		);
	});
});
