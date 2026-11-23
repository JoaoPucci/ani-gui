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

describe('settled', () => {
	/** Whether `p` has resolved once pending callbacks have run. */
	async function resolved(p: Promise<void>) {
		let done = false;
		void p.then(() => (done = true));
		await new Promise((r) => setTimeout(r, 0));
		return done;
	}

	it('waits for the latest load to finish', async () => {
		const loads = createRailLoads();
		const load = loads.start(false);
		const settled = loads.settled();
		expect(await resolved(settled)).toBe(false);
		loads.finish(load);
		expect(await resolved(settled)).toBe(true);
	});

	it('waits past a load that finished superseded, for the one that superseded it', async () => {
		const loads = createRailLoads();
		const refresh = loads.start(false);
		const reread = loads.start(false);
		const settled = loads.settled();
		loads.finish(refresh);
		expect(await resolved(settled)).toBe(false);
		loads.finish(reread);
		expect(await resolved(settled)).toBe(true);
	});

	it('waits for a load that starts while it waits', async () => {
		const loads = createRailLoads();
		const first = loads.start(false);
		const settled = loads.settled();
		const second = loads.start(false);
		loads.finish(first);
		expect(await resolved(settled)).toBe(false);
		loads.finish(second);
		expect(await resolved(settled)).toBe(true);
	});

	it('resolves at once when the latest load has already finished, or none has started', async () => {
		const loads = createRailLoads();
		expect(await resolved(loads.settled())).toBe(true);
		loads.finish(loads.start(false));
		expect(await resolved(loads.settled())).toBe(true);
	});
});
