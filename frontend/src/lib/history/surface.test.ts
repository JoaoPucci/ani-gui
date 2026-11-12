// The surface an in-place link session lives on, left or not.

import { describe, expect, it } from 'vitest';
import { surface } from './surface';

describe('the surface', () => {
	it('is here until it is left, and somewhere else after', () => {
		const place = surface();
		expect(place.gone).toBe(false);
		const here = place.here();
		place.leave();
		expect(place.gone).toBe(true);
		expect(place.here()).not.toBe(here);
	});
});
