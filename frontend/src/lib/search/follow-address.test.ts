import { describe, expect, it } from 'vitest';
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
});
