import { describe, expect, it } from 'vitest';
import * as fc from 'fast-check';

import { runUnairedClick, type UnairedClickDeps } from './unaired-click';
import { createUnairedRecheck, planUnairedClick } from './unaired-recheck';

const CAPTION = 'Checking the schedule…';

/**
 * A route as the coordinator sees it: a busy flag with a caption, a
 * page that learns whether the episode aired from what `apply` stores,
 * and a log of every action taken.
 */
function route(
	opts: {
		airsOnRefresh?: boolean;
		beyondPlayable?: boolean;
		fails?: boolean;
		busy?: boolean;
		show?: string | null;
		episode?: number;
	} = {}
) {
	const log: string[] = [];
	const fetched: [string, number][] = [];
	let busy = opts.busy ?? false;
	let caption: string | null = null;
	let aired = false;
	let context = 'visit-1';
	let release: (() => void) | null = null;
	let hold = false;
	const deps: UnairedClickDeps<boolean> = {
		show: opts.show === undefined ? '49847' : opts.show,
		episode: opts.episode ?? 3,
		kitsuPageSize: 20,
		caption: CAPTION,
		isBusy: () => busy,
		hold: (c) => {
			busy = true;
			caption = c;
		},
		heldCaption: () => caption,
		release: () => {
			busy = false;
			caption = null;
			log.push('release');
		},
		fetch: async (show, page) => {
			fetched.push([show, page]);
			if (hold) await new Promise<void>((r) => (release = r));
			if (opts.fails) throw new Error('network');
			return opts.airsOnRefresh === true;
		},
		apply: (airsNow, page) => {
			log.push(`apply p${page}`);
			aired = airsNow;
		},
		isAired: () => aired,
		beyondPlayable: () => opts.beyondPlayable === true,
		currentContext: () => context,
		play: () => log.push('play'),
		recheckProvider: () => log.push('recheck-provider'),
		notifyStillUnaired: () => log.push('still-unaired'),
		notifyFailed: () => log.push('failed'),
		now: () => 1_000_000,
		recheck: createUnairedRecheck()
	};
	return {
		deps,
		log,
		fetched,
		isBusy: () => busy,
		caption: () => caption,
		leave: () => (context = 'visit-2'),
		takeOver: (c: string) => {
			busy = true;
			caption = c;
		},
		holdFetch: () => (hold = true),
		releaseFetch: () => release?.()
	};
}

describe('runUnairedClick', () => {
	it('plays an episode the refresh shows has aired', async () => {
		const r = route({ airsOnRefresh: true });
		expect(await runUnairedClick(r.deps)).toBe('play');
		expect(r.log).toEqual(['apply p1', 'release', 'play']);
		expect(r.isBusy()).toBe(false);
	});

	it('hands an aired episode past the provider count to the provider re-ask', async () => {
		const r = route({ airsOnRefresh: true, beyondPlayable: true });
		expect(await runUnairedClick(r.deps)).toBe('recheck-provider');
		// Released first: the re-ask raises its own block.
		expect(r.log).toEqual(['apply p1', 'release', 'recheck-provider']);
	});

	it('says an episode still has not aired', async () => {
		const r = route();
		expect(await runUnairedClick(r.deps)).toBe('still-unaired');
		expect(r.log).toEqual(['apply p1', 'release', 'still-unaired']);
	});

	it('says the check failed, applying nothing', async () => {
		const r = route({ fails: true });
		expect(await runUnairedClick(r.deps)).toBe('failed');
		expect(r.log).toEqual(['release', 'failed']);
	});

	it('holds the page with its caption while the check is out', async () => {
		const r = route({ airsOnRefresh: true });
		r.holdFetch();
		const pending = runUnairedClick(r.deps);
		await Promise.resolve();
		expect(r.isBusy()).toBe(true);
		expect(r.caption()).toBe(CAPTION);
		r.releaseFetch();
		await pending;
		expect(r.isBusy()).toBe(false);
	});

	it('does nothing while the page is busy, or before it knows its show', async () => {
		const busy = route({ busy: true });
		expect(await runUnairedClick(busy.deps)).toBe('ignored');
		expect(busy.fetched).toEqual([]);
		const showless = route({ show: null });
		expect(await runUnairedClick(showless.deps)).toBe('ignored');
		expect(showless.fetched).toEqual([]);
		expect(showless.isBusy()).toBe(false);
	});

	it('fetches the Kitsu page the episode sits on', async () => {
		const r = route({ episode: 25 });
		await runUnairedClick(r.deps);
		expect(r.fetched).toEqual([['49847', 2]]);
		expect(r.log[0]).toBe('apply p2');
	});

	it('reads the wall clock when the route gives it none', async () => {
		// The routes pass no clock; the limit then runs on Date.now.
		const r = route({ airsOnRefresh: true });
		const deps = { ...r.deps };
		delete deps.now;
		expect(await runUnairedClick(deps)).toBe('play');
	});

	describe('a check that lands after the user left', () => {
		it('applies nothing, acts on nothing, and lets go of its own block', async () => {
			const r = route({ airsOnRefresh: true });
			r.holdFetch();
			const pending = runUnairedClick(r.deps);
			await Promise.resolve();
			r.leave();
			r.releaseFetch();
			expect(await pending).toBe('none');
			expect(r.log).toEqual(['release']);
			expect(r.isBusy()).toBe(false);
		});

		it('leaves a block another action raised since alone', async () => {
			const r = route({ airsOnRefresh: true });
			r.holdFetch();
			const pending = runUnairedClick(r.deps);
			await Promise.resolve();
			r.leave();
			r.takeOver('Resolving the next show…');
			r.releaseFetch();
			expect(await pending).toBe('none');
			expect(r.log).toEqual([]);
			expect(r.isBusy()).toBe(true);
			expect(r.caption()).toBe('Resolving the next show…');
		});
	});

	describe('joining and limiting', () => {
		it('joins clicks on the same page of a show into one refresh', async () => {
			const recheck = createUnairedRecheck();
			const a = route({ airsOnRefresh: true, episode: 3 });
			const b = route({ airsOnRefresh: true, episode: 4 });
			a.holdFetch();
			const first = runUnairedClick({ ...a.deps, recheck });
			await Promise.resolve();
			const second = runUnairedClick({ ...b.deps, recheck });
			a.releaseFetch();
			await Promise.all([first, second]);
			expect(a.fetched.length + b.fetched.length).toBe(1);
		});

		it('gives each page of a show its own refresh', async () => {
			const recheck = createUnairedRecheck();
			const a = route({ episode: 3 });
			const b = route({ episode: 25 });
			await runUnairedClick({ ...a.deps, recheck });
			await runUnairedClick({ ...b.deps, recheck });
			expect(a.fetched).toEqual([['49847', 1]]);
			expect(b.fetched).toEqual([['49847', 2]]);
		});

		it('answers a repeat click on a page from what it holds', async () => {
			const recheck = createUnairedRecheck();
			const r = route();
			await runUnairedClick({ ...r.deps, recheck });
			expect(await runUnairedClick({ ...r.deps, recheck })).toBe('still-unaired');
			expect(r.fetched).toHaveLength(1);
		});
	});
});

describe('planUnairedClick — properties', () => {
	const outcome = fc.constantFrom('aired', 'unaired', 'failed', 'superseded' as const);

	it('acts on an aired episode as an aired tile would, and on nothing else', () => {
		fc.assert(
			fc.property(outcome, fc.boolean(), (o, beyond) => {
				const plan = planUnairedClick(o, beyond);
				const acts = plan === 'play' || plan === 'recheck-provider';
				expect(acts).toBe(o === 'aired');
				if (o === 'aired') expect(plan).toBe(beyond ? 'recheck-provider' : 'play');
			})
		);
	});

	it('says nothing at all only for a page the user left', () => {
		fc.assert(
			fc.property(outcome, fc.boolean(), (o, beyond) => {
				expect(planUnairedClick(o, beyond) === 'none').toBe(o === 'superseded');
			})
		);
	});
});
