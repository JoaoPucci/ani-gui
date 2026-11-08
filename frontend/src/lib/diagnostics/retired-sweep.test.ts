/**
 * The diagnostics page's rendering decision for the retired-provider
 * sweep, kept out of the `.svelte` file so a test can reach it.
 */
import { describe, it, expect } from 'vitest';
import { retiredSweepView } from './retired-sweep';

describe('retiredSweepView', () => {
	it('names the rows it removed and the ones it rewrote', () => {
		const view = retiredSweepView({
			superseded: ['Naruto Shippuden'],
			renumbered: ['The Separation'],
			numbering_rows: 2,
			cache_rows: 51
		});
		expect(view).toEqual({
			visible: true,
			superseded: ['Naruto Shippuden'],
			renumbered: ['The Separation'],
			storedEntries: 53
		});
	});

	it('shows the block when only stored entries went', () => {
		const view = retiredSweepView({
			superseded: [],
			renumbered: [],
			numbering_rows: 0,
			cache_rows: 3
		});
		expect(view.visible).toBe(true);
		expect(view.storedEntries).toBe(3);
	});

	it('stays hidden when the sweep changed nothing', () => {
		expect(
			retiredSweepView({ superseded: [], renumbered: [], numbering_rows: 0, cache_rows: 0 }).visible
		).toBe(false);
	});

	it('reads a backend that predates the field as nothing to show', () => {
		// A development run against a stale debug binary has no field.
		expect(retiredSweepView(undefined).visible).toBe(false);
		expect(retiredSweepView({}).visible).toBe(false);
	});
});
