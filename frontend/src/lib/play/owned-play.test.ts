import { describe, expect, it, vi } from 'vitest';
import { altTitlesFromKitsu, type KitsuAnimeRef } from '$lib/api';
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
		const describe = vi.fn(async () => {
			// The viewer moves on while the message is being worded.
			visits.move();
			return 'the message';
		});
		const out = await runOwnedPlay(
			owner,
			async () => {
				throw new Error('no stream');
			},
			describe
		);
		expect(describe).toHaveBeenCalled();
		expect(out).toEqual({ kind: 'abandoned' });
	});

	it('lets a failure to word a failure through, as the pages did', async () => {
		const owner = claimPlay('42', routeVisits().read);
		await expect(
			runOwnedPlay(
				owner,
				async () => {
					throw new Error('no stream');
				},
				async () => {
					throw new Error('wording broke');
				}
			)
		).rejects.toThrow('wording broke');
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

describe('a play whose page was destroyed', () => {
	// Leaving the route destroys the page without changing its show id,
	// so the visit counter does not move; the claim must still see it.
	function page() {
		let gone = false;
		return {
			gone: () => gone,
			destroy: () => {
				gone = true;
			}
		};
	}

	it('is no longer current once the page is destroyed on the same visit', () => {
		const p = page();
		const owner = claimPlay('42', routeVisits().read, p.gone);
		expect(owner.isCurrent()).toBe(true);
		p.destroy();
		expect(owner.isCurrent()).toBe(false);
	});

	it('reports a success after the page was destroyed as not current', async () => {
		const p = page();
		const owner = claimPlay('42', routeVisits().read, p.gone);
		const play = deferred<string>();
		const pending = runOwnedPlay(owner, () => play.promise, vi.fn());
		p.destroy();
		play.resolve('session-1');
		expect(await pending).toEqual({ kind: 'played', session: 'session-1', current: false });
	});

	it('abandons a failure after the page was destroyed, without wording it', async () => {
		const p = page();
		const owner = claimPlay('42', routeVisits().read, p.gone);
		const play = deferred<string>();
		const describe = vi.fn(async () => 'x');
		const pending = runOwnedPlay(owner, () => play.promise, describe);
		p.destroy();
		play.reject(new Error('no stream'));
		expect(await pending).toEqual({ kind: 'abandoned' });
		expect(describe).not.toHaveBeenCalled();
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
		expect(args.alt_titles).toEqual(altTitlesFromKitsu(show));
		expect(args.alt_titles).toContain('Show EN');
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
		expect('kitsu_id' in args).toBe(false);
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
