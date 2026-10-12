import { afterEach, describe, expect, it } from 'vitest';
import { m } from '$lib/paraglide/messages';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';
import { rangeError } from './range-error';

const range = (startEp: number, endEp: number, maxEpisode: number | null, rangeMax = 200) => ({
	mode: 'range' as const,
	startEp,
	endEp,
	rangeMax: maxEpisode ?? rangeMax,
	maxEpisode
});

// The unit tier runs in node, where the runtime's localStorage
// strategy has nothing to read and `setLocale` cannot take effect, so
// the locale case overrides the lookup itself and restores it after.
const originalGetLocale = getLocale;
afterEach(() => {
	overwriteGetLocale(originalGetLocale);
});

describe('rangeError', () => {
	it('checks only Range mode', () => {
		expect(rangeError({ ...range(0, -5, 12), mode: 'this' })).toBeNull();
		expect(rangeError({ ...range(0, -5, 12), mode: 'all' })).toBeNull();
	});

	it('accepts a range inside the known count', () => {
		expect(rangeError(range(1, 12, 12))).toBeNull();
		expect(rangeError(range(3, 3, 12))).toBeNull();
	});

	it('names each way a range can be wrong, through the message catalogue', () => {
		expect(rangeError(range(Number.NaN, 4, 12))).toBe(m.download_range_error_not_a_number());
		expect(rangeError(range(0, 4, 12))).toBe(m.download_range_error_below_one());
		expect(rangeError(range(13, 14, 12))).toBe(
			m.download_range_error_start_over_max({ max: '12' })
		);
		expect(rangeError(range(2, 13, 12))).toBe(m.download_range_error_end_over_max({ max: '12' }));
		expect(rangeError(range(5, 2, 12))).toBe(m.download_range_error_end_before_start());
	});

	it('names the cap instead of a count when the count is unknown', () => {
		expect(rangeError(range(1, 201, null))).toBe(m.download_range_error_over_cap({ max: '200' }));
		expect(rangeError(range(201, 202, null))).toBe(m.download_range_error_over_cap({ max: '200' }));
	});

	it('follows the locale', () => {
		overwriteGetLocale(() => 'pt-BR');
		expect(rangeError(range(5, 2, 12))).toBe(
			m.download_range_error_end_before_start({}, { locale: 'pt-BR' })
		);
		expect(rangeError(range(5, 2, 12))).not.toBe(
			m.download_range_error_end_before_start({}, { locale: 'en' })
		);
	});
});
