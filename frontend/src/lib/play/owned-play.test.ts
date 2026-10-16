import { describe, expect, it, vi } from 'vitest';
import type { KitsuAnimeRef } from '$lib/api';
import { claimPlay, playArgsFor, runOwnedPlay } from './owned-play';

/** A route's visit counter, as the pages keep it: it advances on every
 *  change of the show id, including a round trip back. */
function routeVisits() {
	let visit = 1;
	return {
		read: () => visit,
		move: () => {
			visit += 1;
		}
	};
}

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

describe('claimPlay', () => {
	it('keeps the show and visit the play was started for', () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		expect(owner.showId).toBe('42');
		expect(owner.visit).toBe(1);
		expect(owner.isCurrent()).toBe(true);
	});

	it('is no longer current once the route moves, even back to the same show', () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		visits.move(); // to another show
		expect(owner.isCurrent()).toBe(false);
		visits.move(); // and back to 42: a later visit, not this one
		expect(owner.isCurrent()).toBe(false);
		expect(owner.showId).toBe('42');
	});
});

describe('runOwnedPlay', () => {
	it('reports a play that answers on the same visit as played and current', async () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const describe = vi.fn(async () => 'never');
		const out = await runOwnedPlay(owner, async () => 'session-1', describe);
		expect(out).toEqual({ kind: 'played', session: 'session-1', current: true });
		expect(describe).not.toHaveBeenCalled();
	});

	it('reports a play that answers after the route moved as played, not current', async () => {
		// The watch is still the clicked show's; the page decides not to
		// open its player.
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const play = deferred<string>();
		const pending = runOwnedPlay(owner, () => play.promise, vi.fn());
		visits.move();
		play.resolve('session-1');
		expect(await pending).toEqual({ kind: 'played', session: 'session-1', current: false });
	});

	it('words a failure on the same visit', async () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const err = { kind: 'episode_unavailable' };
		const describe = vi.fn(async () => 'the message');
		const out = await runOwnedPlay(
			owner,
			async () => {
				throw err;
			},
			describe
		);
		expect(out).toEqual({ kind: 'failed', error: err, message: 'the message' });
		expect(describe).toHaveBeenCalledWith(err);
	});

	it('abandons a failure that lands after the route moved, without wording it', async () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const play = deferred<string>();
		const describe = vi.fn(async () => 'the message');
		const pending = runOwnedPlay(owner, () => play.promise, describe);
		visits.move();
		play.reject(new Error('no stream'));
		expect(await pending).toEqual({ kind: 'abandoned' });
		// Wording asks the cache; nobody is on that visit to read it.
		expect(describe).not.toHaveBeenCalled();
	});

	it('abandons a failure when the route moves while it is being worded', async () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const worded = deferred<string>();
		const pending = runOwnedPlay(
			owner,
			async () => {
				throw new Error('no stream');
			},
			() => worded.promise
		);
		await Promise.resolve();
		visits.move();
		worded.resolve('the message');
		expect(await pending).toEqual({ kind: 'abandoned' });
	});

	it('abandons a round trip back to the same show too', async () => {
		const visits = routeVisits();
		const owner = claimPlay('42', visits.read);
		const play = deferred<string>();
		const pending = runOwnedPlay(
			owner,
			() => play.promise,
			vi.fn(async () => 'x')
		);
		visits.move();
		visits.move();
		play.reject(new Error('no stream'));
		expect(await pending).toEqual({ kind: 'abandoned' });
	});
});

describe('playArgsFor', () => {
	const show = {
		id: '42',
		canonical_title: 'Show',
		episode_count: 12,
		subtype: 'TV',
		start_date: '2019-04-01',
		titles: { en: 'Show EN' },
		abbreviated_titles: []
	} as unknown as KitsuAnimeRef;

	it('builds the play request from the show as it was at the click', () => {
		const args = playArgsFor({
			title: 'Show',
			episode: 5,
			mode: 'dub',
			quality: '1080',
			show,
			kitsuId: '42'
		});
		expect(args).toMatchObject({
			title: 'Show',
			episode: '5',
			mode: 'dub',
			quality: '1080',
			episode_count: 12,
			year: 2019,
			subtype: 'TV',
			kitsu_id: '42'
		});
		expect(Array.isArray(args.alt_titles)).toBe(true);
	});

	it('carries no Kitsu id when the play records none', () => {
		const args = playArgsFor({
			title: 'Show',
			episode: 5,
			mode: 'sub',
			quality: 'best',
			show,
			kitsuId: undefined
		});
		expect(args.kitsu_id).toBeUndefined();
	});

	it('takes a missing show as no metadata', () => {
		const args = playArgsFor({
			title: 'Show',
			episode: 1,
			mode: 'sub',
			quality: 'best',
			show: null,
			kitsuId: '42'
		});
		expect(args).toMatchObject({ episode_count: null, year: null, subtype: null });
	});
});
