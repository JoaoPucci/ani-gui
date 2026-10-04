import { describe, expect, it } from 'vitest';

import {
	createUnairedRecheck,
	planUnairedClick,
	UNAIRED_RECHECK_INTERVAL_MS,
	type UnairedRecheckDeps
} from './unaired-recheck';

/** A page whose schedule says the episode is out once refreshed. */
function page(opts: { airsOnRefresh?: boolean; fails?: boolean } = {}) {
	let aired = false;
	let refreshes = 0;
	let clock = 1_000_000;
	let context = 'visit-1';
	let release: (() => void) | null = null;
	let hold = false;
	const deps: UnairedRecheckDeps = {
		refresh: async () => {
			refreshes += 1;
			if (hold) await new Promise<void>((r) => (release = r));
			if (opts.fails) throw new Error('network');
			if (opts.airsOnRefresh) aired = true;
		},
		isAired: () => aired,
		currentContext: () => context,
		now: () => clock
	};
	return {
		deps,
		refreshes: () => refreshes,
		advance: (ms: number) => (clock += ms),
		leave: () => (context = 'visit-2'),
		holdRefresh: () => (hold = true),
		releaseRefresh: () => release?.()
	};
}

describe('createUnairedRecheck', () => {
	it('refreshes the schedule and reports the episode aired when it now is', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		expect(await recheck.check('49847', p.deps)).toBe('aired');
		expect(p.refreshes()).toBe(1);
	});

	it('reports it still unaired when the refreshed schedule agrees', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		expect(await recheck.check('49847', p.deps)).toBe('unaired');
		expect(p.refreshes()).toBe(1);
	});

	it('answers a repeat click from what the page holds until the interval passes', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		await recheck.check('49847', p.deps);
		p.advance(UNAIRED_RECHECK_INTERVAL_MS - 1);
		expect(await recheck.check('49847', p.deps)).toBe('unaired');
		expect(p.refreshes()).toBe(1);
		p.advance(1);
		await recheck.check('49847', p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('limits each show on its own', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		await recheck.check('49847', p.deps);
		await recheck.check('50551', p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('shares one refresh between clicks that land while it is out', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		p.holdRefresh();
		const first = recheck.check('49847', p.deps);
		const second = recheck.check('49847', p.deps);
		await Promise.resolve();
		p.releaseRefresh();
		expect(await first).toBe('aired');
		expect(await second).toBe('aired');
		expect(p.refreshes()).toBe(1);
	});

	it('reports a failed refresh, and does not hold the next click back for it', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ fails: true });
		expect(await recheck.check('49847', p.deps)).toBe('failed');
		await recheck.check('49847', p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('says nothing about a page the user has left', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		p.holdRefresh();
		const pending = recheck.check('49847', p.deps);
		await Promise.resolve();
		p.leave();
		p.releaseRefresh();
		expect(await pending).toBe('superseded');
	});
});

describe('planUnairedClick', () => {
	it('plays an episode that turned out aired and is in the catalogue', () => {
		expect(planUnairedClick('aired', false)).toBe('play');
	});

	it('asks the provider about one that aired past its catalogued count', () => {
		expect(planUnairedClick('aired', true)).toBe('recheck-provider');
	});

	it('says so when it still has not aired, or the check failed', () => {
		expect(planUnairedClick('unaired', false)).toBe('still-unaired');
		expect(planUnairedClick('failed', false)).toBe('failed');
	});

	it('does nothing for a page the user left', () => {
		expect(planUnairedClick('superseded', true)).toBe('none');
	});
});
