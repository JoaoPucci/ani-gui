import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { franchiseTrail } from './franchise-page';

describe('franchiseTrail', () => {
	it('reads Home, the show it was opened from, then Franchise', () => {
		expect(
			franchiseTrail({
				opened_from: { id: '41410', title: "JoJo's Bizarre Adventure: Golden Wind" }
			})
		).toEqual([
			{ label: 'Home', href: '/' },
			{ label: "JoJo's Bizarre Adventure: Golden Wind", href: '/anime/41410' },
			{ label: 'Franchise' }
		]);
	});

	it('reads Home, then Franchise, where the page does not list the show', () => {
		expect(franchiseTrail({ opened_from: null })).toEqual([
			{ label: 'Home', href: '/' },
			{ label: 'Franchise' }
		]);
	});

	it('always ends on Franchise, with the show between Home and it when the page lists it', () => {
		fc.assert(
			fc.property(
				fc.option(fc.record({ id: fc.stringMatching(/^\d{1,5}$/), title: fc.string() }), {
					nil: null
				}),
				(from) => {
					const trail = franchiseTrail({ opened_from: from });
					expect(trail[0]).toEqual({ label: 'Home', href: '/' });
					expect(trail.at(-1)).toEqual({ label: 'Franchise' });
					expect(trail).toHaveLength(from ? 3 : 2);
					if (from) expect(trail[1]).toEqual({ label: from.title, href: `/anime/${from.id}` });
				}
			)
		);
	});
});
