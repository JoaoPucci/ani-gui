import { afterEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { formatSpeed, speedParts, totalSpeed } from './speed';
import { m } from '$lib/paraglide/messages';
import { getLocale, overwriteGetLocale } from '$lib/paraglide/runtime';

const MIB = 1024 * 1024;

describe('speedParts', () => {
	it('reads under a mebibyte a second as whole kilobytes', () => {
		expect(speedParts(512 * 1024)).toEqual({ unit: 'kbps', value: 512 });
		expect(speedParts(0)).toEqual({ unit: 'kbps', value: 0 });
	});

	it('reads a mebibyte a second and above as megabytes to one decimal', () => {
		expect(speedParts(1.5 * MIB)).toEqual({ unit: 'mbps', value: 1.5 });
		expect(speedParts(12.34 * MIB)).toEqual({ unit: 'mbps', value: 12.3 });
	});

	it('never reads as negative, and the unit follows the size', () => {
		fc.assert(
			fc.property(fc.double({ min: -1e3, max: 1e10, noNaN: true }), (bps) => {
				const { unit, value } = speedParts(bps);
				expect(value).toBeGreaterThanOrEqual(0);
				expect(unit).toBe(bps >= MIB ? 'mbps' : 'kbps');
			})
		);
	});
});

describe('totalSpeed', () => {
	it('adds the speeds of the downloads still running', () => {
		expect(
			totalSpeed([
				{ status: 'active', speed: 1000 },
				{ status: 'active', speed: 500 },
				{ status: 'done', speed: 9999 },
				{ status: 'pending', speed: null }
			])
		).toBe(1500);
	});

	it('reads a running download that has not reported yet as zero', () => {
		// A download the user just started is running even before its
		// first bytes; showing nothing read as the feature missing.
		expect(totalSpeed([{ status: 'active', speed: null }])).toBe(0);
		expect(totalSpeed([{ status: 'pending', speed: null }])).toBe(0);
	});

	it('is nothing while no download is running', () => {
		expect(totalSpeed([])).toBeNull();
		expect(totalSpeed([{ status: 'error', speed: 4000 }])).toBeNull();
		expect(totalSpeed([{ status: 'done', speed: 4000 }])).toBeNull();
	});
});

describe('formatSpeed', () => {
	const initialLocale = getLocale();
	afterEach(() => overwriteGetLocale(() => initialLocale));

	it('reads whole kilobytes under a mebibyte a second', () => {
		overwriteGetLocale(() => 'en');
		expect(formatSpeed(512 * 1024)).toBe(m.download_speed_kbps({ value: '512' }));
		expect(formatSpeed(0)).toBe(m.download_speed_kbps({ value: '0' }));
	});

	it('reads megabytes to one decimal from a mebibyte a second up', () => {
		overwriteGetLocale(() => 'en');
		expect(formatSpeed(1.5 * 1024 * 1024)).toBe(m.download_speed_mbps({ value: '1.5' }));
		expect(formatSpeed(2 * 1024 * 1024)).toBe(m.download_speed_mbps({ value: '2' }));
	});

	it("writes the number in the reader's locale", () => {
		overwriteGetLocale(() => 'pt-BR');
		expect(formatSpeed(1.5 * 1024 * 1024)).toBe(m.download_speed_mbps({ value: '1,5' }));
		overwriteGetLocale(() => 'en');
		expect(formatSpeed(2048 * 1024 * 1024)).toBe(m.download_speed_mbps({ value: '2,048' }));
	});
});
