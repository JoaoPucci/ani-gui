import { afterEach, describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import { subtypeLabel } from './subtype-label';

// Node has no localStorage for the runtime's locale strategy, so the
// locale case overrides the lookup and restores it afterwards.
const originalGetLocale = getLocale;
afterEach(() => {
	overwriteGetLocale(originalGetLocale);
});

describe('subtypeLabel', () => {
	it('labels each Kitsu subtype from the message catalogue, upper-cased', () => {
		expect(subtypeLabel('TV')).toBe(m.app_subtype_tv().toUpperCase());
		expect(subtypeLabel('movie')).toBe(m.app_subtype_movie().toUpperCase());
		expect(subtypeLabel('special')).toBe(m.app_subtype_special().toUpperCase());
		expect(subtypeLabel('OVA')).toBe(m.app_subtype_ova().toUpperCase());
		expect(subtypeLabel('ONA')).toBe(m.app_subtype_ona().toUpperCase());
		expect(subtypeLabel('music')).toBe(m.app_subtype_music().toUpperCase());
	});

	it('reads a missing subtype as TV', () => {
		expect(subtypeLabel(null)).toBe(subtypeLabel('TV'));
		expect(subtypeLabel(undefined)).toBe(subtypeLabel('TV'));
	});

	it('follows the locale', () => {
		overwriteGetLocale(() => 'pt-BR');
		expect(subtypeLabel('movie')).toBe('FILME');
		overwriteGetLocale(() => 'ru');
		expect(subtypeLabel('movie')).toBe('ФИЛЬМ');
		expect(subtypeLabel('TV')).toBe('ТВ');
	});

	it('shows a subtype it does not know as Kitsu sent it, upper-cased', () => {
		expect(subtypeLabel('web')).toBe('WEB');
	});
});
