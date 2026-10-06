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
	let applied = 0;
	const deps: UnairedRecheckDeps<boolean> = {
		// The refresh only fetches; what it fetched reaches the page
		// through `apply`, and only while the page is the one that asked.
		refresh: async () => {
			refreshes += 1;
			if (hold) await new Promise<void>((r) => (release = r));
			if (opts.fails) throw new Error('network');
			return opts.airsOnRefresh === true;
		},
		apply: (airsNow) => {
			applied += 1;
			aired = airsNow;
		},
		isAired: () => aired,
		currentContext: () => context,
		now: () => clock
	};
	return {
		deps,
		refreshes: () => refreshes,
		applied: () => applied,
		aired: () => aired,
		advance: (ms: number) => (clock += ms),
		leave: () => (context = 'visit-2'),
		holdRefresh: () => (hold = true),
		stopHolding: () => (hold = false),
		releaseRefresh: () => release?.()
	};
}

describe('createUnairedRecheck', () => {
	it('refreshes the schedule and reports the episode aired when it now is', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		expect(await recheck.check({ show: '49847', page: 1 }, p.deps)).toBe('aired');
		expect(p.refreshes()).toBe(1);
	});

	it('reports it still unaired when the refreshed schedule agrees', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		expect(await recheck.check({ show: '49847', page: 1 }, p.deps)).toBe('unaired');
		expect(p.refreshes()).toBe(1);
	});

	it('answers a repeat click from what the page holds until the interval passes', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		p.advance(UNAIRED_RECHECK_INTERVAL_MS - 1);
		expect(await recheck.check({ show: '49847', page: 1 }, p.deps)).toBe('unaired');
		expect(p.refreshes()).toBe(1);
		p.advance(1);
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('limits each show on its own', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		await recheck.check({ show: '50551', page: 1 }, p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('asks again for another page of the same show, and gives each page its own answer', async () => {
		// The controller is app-wide and a show's pages are separate
		// fetches: a click for page 1 joining a refresh out for page 2
		// would write page 2's episodes in as page 1's.
		const recheck = createUnairedRecheck();
		const applied: string[] = [];
		let release!: () => void;
		const held = new Promise<void>((r) => (release = r));
		const deps = (page: string): UnairedRecheckDeps<string> => ({
			refresh: async () => {
				await held;
				return page;
			},
			apply: (fetched) => applied.push(`${page}<-${fetched}`),
			isAired: () => false,
			currentContext: () => 'visit-1',
			now: () => 1_000_000
		});
		const two = recheck.check({ show: '49847', page: 2 }, deps('p2'));
		const one = recheck.check({ show: '49847', page: 1 }, deps('p1'));
		release();
		await Promise.all([one, two]);
		expect(applied.sort()).toEqual(['p1<-p1', 'p2<-p2']);
	});

	it('limits each page of a show on its own', async () => {
		const recheck = createUnairedRecheck();
		const p = page();
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		await recheck.check({ show: '49847', page: 2 }, p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('shares one refresh between clicks that land while it is out', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		p.holdRefresh();
		const first = recheck.check({ show: '49847', page: 1 }, p.deps);
		const second = recheck.check({ show: '49847', page: 1 }, p.deps);
		await Promise.resolve();
		p.releaseRefresh();
		expect(await first).toBe('aired');
		expect(await second).toBe('aired');
		expect(p.refreshes()).toBe(1);
	});

	it('reports a failed refresh, and does not hold the next click back for it', async () => {
		const recheck = createUnairedRecheck();
		const p = page({ fails: true });
		expect(await recheck.check({ show: '49847', page: 1 }, p.deps)).toBe('failed');
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		expect(p.refreshes()).toBe(2);
	});

	it('says nothing about a page the user has left, and writes nothing into it', async () => {
		// The route component outlives the show: whatever the refresh
		// fetched for the show the user left must not land in the page
		// that now shows another one.
		const recheck = createUnairedRecheck();
		const p = page({ airsOnRefresh: true });
		p.holdRefresh();
		const pending = recheck.check({ show: '49847', page: 1 }, p.deps);
		await Promise.resolve();
		p.leave();
		p.releaseRefresh();
		expect(await pending).toBe('superseded');
		expect(p.applied()).toBe(0);
		expect(p.aired()).toBe(false);
	});

	it('does not start the limit for a refresh nobody applied', async () => {
		// The user left before the answer landed, so no page holds it;
		// answering the next click from page state would answer from the
		// stale cache the click is questioning.
		const recheck = createUnairedRecheck();
		const p = page();
		p.holdRefresh();
		const pending = recheck.check({ show: '49847', page: 1 }, p.deps);
		await Promise.resolve();
		p.leave();
		p.releaseRefresh();
		await pending;
		p.stopHolding();
		await recheck.check({ show: '49847', page: 1 }, p.deps);
		expect(p.refreshes()).toBe(2);
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
