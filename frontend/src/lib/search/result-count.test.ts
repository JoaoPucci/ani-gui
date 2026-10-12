import { afterEach, describe, expect, it } from 'vitest';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import { resultCount } from './result-count';

// Node has no localStorage for the runtime's locale strategy, so each
// locale case overrides the lookup and restores it afterwards.
const originalGetLocale = getLocale;
afterEach(() => {
	overwriteGetLocale(originalGetLocale);
});

describe('resultCount', () => {
	it('shows the whole count once Kitsu has no more pages', () => {
		overwriteGetLocale(() => 'en');
		expect(resultCount(1, false)).toEqual({ num: '1', word: 'result' });
		expect(resultCount(7, false)).toEqual({ num: '7', word: 'results' });
		expect(resultCount(0, false)).toEqual({ num: '0', word: 'results' });
	});

	it('marks the count with "+" while more pages are to come', () => {
		overwriteGetLocale(() => 'en');
		expect(resultCount(20, true)).toEqual({ num: '20+', word: 'results' });
	});

	it('shows a bare zero while the first pages are empty', () => {
		overwriteGetLocale(() => 'en');
		expect(resultCount(0, true).num).toBe('0');
	});

	it('counts in the locale', () => {
		overwriteGetLocale(() => 'ru');
		expect(resultCount(5, false)).toEqual({ num: '5', word: 'результатов' });
		expect(resultCount(22, false)).toEqual({ num: '22', word: 'результата' });
	});
});
