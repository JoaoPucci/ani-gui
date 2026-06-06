'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { extractLocaleFromToml } = require('./extract-locale-from-toml');

test('extracts a top-level locale string', () => {
	assert.equal(extractLocaleFromToml('locale = "pt-BR"\n'), 'pt-BR');
});

test('tolerates surrounding whitespace and other keys', () => {
	const toml = [
		'# config.toml',
		'',
		'mode = "sub"',
		'  locale  =  "es-419"  ',
		'quality = "best"'
	].join('\n');
	assert.equal(extractLocaleFromToml(toml), 'es-419');
});

test('returns null when locale key is missing', () => {
	// Settings page never wrote a locale (fresh install), or the
	// user hand-edited the file and deleted the key. Preload must
	// fall through to Paraglide's preferredLanguage / baseLocale
	// strategies instead of seeding localStorage with garbage.
	assert.equal(extractLocaleFromToml('mode = "sub"\nquality = "best"\n'), null);
});

test('returns null on empty input', () => {
	assert.equal(extractLocaleFromToml(''), null);
});

test('does not match `locale` keys nested under a TOML table header', () => {
	// `[scraper] \n locale = "..."` is a different key in TOML's data
	// model. The regex anchors on line-start at the top level only;
	// a nested `locale` belongs to whatever section it follows and
	// should NOT be treated as the UI locale.
	const toml = ['[scraper]', 'locale = "ja"', ''].join('\n');
	assert.equal(extractLocaleFromToml(toml), null);
});

test('handles an empty quoted value by returning the empty string', () => {
	// A user manually wrote `locale = ""`. The caller decides what
	// to do (treat as "no preference" and fall through). The parser
	// itself reports the value as-written.
	assert.equal(extractLocaleFromToml('locale = ""\n'), '');
});
