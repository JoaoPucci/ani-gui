import { describe, expect, it } from 'vitest';

import { createRailLoads } from './watch-later-loads';

describe('createRailLoads', () => {
	it('lets only the latest load write, and runs the opening check when it lands', () => {
		const loads = createRailLoads();
		const opening = loads.start(true);
		expect(loads.current(opening)).toBe(true);
		expect(loads.landed(opening)).toBe(true);
	});

	it('never runs the check for a load a newer one superseded', () => {
		const loads = createRailLoads();
		const opening = loads.start(true);
		loads.start(false);
		expect(loads.current(opening)).toBe(false);
		expect(loads.landed(opening)).toBe(false);
	});

	it('hands the opening check to the load that superseded it', () => {
		const loads = createRailLoads();
		loads.start(true);
		// A settled write rereads the row before the opening load landed.
		const reread = loads.start(false);
		expect(loads.landed(reread)).toBe(true);
	});

	it('runs the check once, not again for later loads', () => {
		const loads = createRailLoads();
		expect(loads.landed(loads.start(true))).toBe(true);
		expect(loads.landed(loads.start(false))).toBe(false);
	});

	it('keeps the check owed past a load that failed, for the next one to land', () => {
		const loads = createRailLoads();
		loads.start(true); // failed: never reported as landed
		expect(loads.landed(loads.start(false))).toBe(true);
	});

	it('owes nothing to loads that never asked for the check', () => {
		const loads = createRailLoads();
		expect(loads.landed(loads.start(false))).toBe(false);
	});
});
