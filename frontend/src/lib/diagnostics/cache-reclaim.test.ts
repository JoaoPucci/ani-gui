/**
 * The diagnostics page's rendering decision for the launch's cache
 * reclaim, kept out of the `.svelte` file so a test can reach it.
 */
import { describe, it, expect } from 'vitest';
import { cacheReclaimView } from './cache-reclaim';

describe('cacheReclaimView', () => {
	it('counts the old and the expired rows the launch deleted', () => {
		expect(cacheReclaimView({ undeclared_rows: 264, expired_rows: 735 })).toEqual({
			visible: true,
			oldRows: 264,
			expiredRows: 735
		});
	});

	it('stays hidden when the launch deleted nothing', () => {
		expect(cacheReclaimView({ undeclared_rows: 0, expired_rows: 0 }).visible).toBe(false);
	});

	it('reads a backend that predates the field as nothing to show', () => {
		expect(cacheReclaimView(undefined).visible).toBe(false);
		expect(cacheReclaimView({}).visible).toBe(false);
	});
});
