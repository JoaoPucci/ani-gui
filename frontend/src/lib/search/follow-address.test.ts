import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { followAddress } from './follow-address';

describe('followAddress', () => {
	it('searches for a query the page is not showing', () => {
		expect(followAddress('frieren', '')).toBe('search');
		expect(followAddress('himmel', 'frieren')).toBe('search');
	});

	it('stays on the query it is already showing', () => {
		expect(followAddress('frieren', 'frieren')).toBe('stay');
	});

	it('stays empty with nothing asked', () => {
		expect(followAddress('', '')).toBe('stay');
	});

	it('clears the search on screen when the address names none', () => {
		// The rail's Search link, or a back press onto the bare search
		// page, while a search is showing.
		expect(followAddress('', 'frieren')).toBe('clear');
	});

	it('leaves the page showing what the address names', () => {
		fc.assert(
			fc.property(fc.string(), fc.string(), (q, submitted) => {
				const step = followAddress(q, submitted);
				const after = step === 'search' ? q : step === 'clear' ? '' : submitted;
				expect(after).toBe(q);
			})
		);
	});
});
