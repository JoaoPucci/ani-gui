import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CreateSessionResponse, PlayProgress } from '$lib/api';
import {
	__resetPlayCacheForTests,
	clearBackgroundForOtherVersions,
	clearBackgroundForShow,
	clearForShow,
	getOrFire,
	makeKey
} from './play-cache';

function fakeResp(seed: string): CreateSessionResponse {
	return {
		session_id: seed,
		media_url: `http://x/s/${seed}/master.m3u8`,
		media_kind: 'hls'
	};
}

/** A `fire` that doesn't emit progress — equivalent to the legacy
 *  signature. Keeps tests focused on caching behaviour. */
function plainFire(
	resp: CreateSessionResponse | Error
): (emit: (p: PlayProgress) => void) => Promise<CreateSessionResponse> {
	return () => (resp instanceof Error ? Promise.reject(resp) : Promise.resolve(resp));
}

afterEach(() => {
	__resetPlayCacheForTests();
});

describe('makeKey', () => {
	it('is stable for the same inputs', () => {
		expect(makeKey('show-1', 1, 'sub', 'best')).toBe(makeKey('show-1', 1, 'sub', 'best'));
	});

	it('differs across each axis', () => {
		const base = makeKey('show-1', 1, 'sub', 'best');
		expect(makeKey('show-2', 1, 'sub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 2, 'sub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 1, 'dub', 'best')).not.toBe(base);
		expect(makeKey('show-1', 1, 'sub', '1080')).not.toBe(base);
	});

	it('keeps a session warmed for one version from serving another', () => {
		// A show the provider lists twice plays a different listing per
		// version: a warm for the uncensored copy must never answer a
		// click for the standard one, nor one naming no version.
		const named = makeKey('show-1', 1, 'sub', 'best', 'uncensored');
		expect(makeKey('show-1', 1, 'sub', 'best', 'standard')).not.toBe(named);
		expect(makeKey('show-1', 1, 'sub', 'best')).not.toBe(named);
		expect(makeKey('show-1', 1, 'sub', 'best', 'standard')).toBe(
			makeKey('show-1', 1, 'sub', 'best', 'standard')
		);
	});

	it("still lets clearForShow release every version's entries", async () => {
		const signals: AbortSignal[] = [];
		const hold = (_emit: unknown, signal: AbortSignal) => {
			signals.push(signal);
			return new Promise<CreateSessionResponse>(() => {});
		};
		void getOrFire(makeKey('show-1', 1, 'sub', 'best', 'standard'), hold).catch(() => {});
		void getOrFire(makeKey('show-1', 1, 'sub', 'best', 'uncensored'), hold).catch(() => {});
		await Promise.resolve();
		await Promise.resolve();
		clearForShow('show-1');
		expect(signals).toHaveLength(2);
		expect(signals.every((s) => s.aborted)).toBe(true);
	});
});

describe('getOrFire', () => {
	it('returns the same in-flight promise for repeated calls', async () => {
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const p1 = getOrFire(k, fire);
		const p2 = getOrFire(k, fire);

		expect(p1).toBe(p2);
		await p1;
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('caches a successful resolution so a later call returns instantly', async () => {
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show-1', 1, 'sub', 'best');

		const first = await getOrFire(k, fire);
		const second = await getOrFire(k, fire);

		expect(second).toEqual(first);
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('promotes a queued prefetch to the foreground fire on click', async () => {
		// A detail-page warm registers the entry with a prefetch:true
		// closure. When the user clicks before it starts, the click's
		// own foreground closure must replace it — otherwise the request
		// still runs as Background and the backend's scraper gate can
		// refuse it (AniError::Network) instead of taking the
		// interactive bypass a click is promised.
		const never = () => new Promise<CreateSessionResponse>(() => {});
		// Saturate the concurrency cap so the prefetch entry queues
		// without starting.
		void getOrFire(makeKey('sat1', 1, 'sub', 'best'), never);
		void getOrFire(makeKey('sat2', 1, 'sub', 'best'), never);
		const key = makeKey('show', 3, 'sub', 'best');
		const prefetchFire = vi.fn(never);
		void getOrFire(key, prefetchFire);
		const foregroundFire = vi.fn(async () => ({ session_id: 'fg' }) as CreateSessionResponse);
		const resolved = await getOrFire(key, foregroundFire, () => {});
		expect(resolved.session_id).toBe('fg');
		expect(foregroundFire).toHaveBeenCalledTimes(1);
		expect(prefetchFire).not.toHaveBeenCalled();
	});

	it('refires a started prefetch as foreground when it fails under a click', async () => {
		// Promotion only helps before the fire starts. When a click
		// attaches to an in-flight prefetch whose backend request runs
		// as Background, a breaker refusal fails that promise — the
		// click must transparently refire once with its own foreground
		// closure instead of surfacing Network for a user action.
		const key = makeKey('show', 4, 'sub', 'best');
		let rejectPrefetch: (e: Error) => void = () => {};
		const prefetchFire = vi.fn(
			() =>
				new Promise<CreateSessionResponse>((_, reject) => {
					rejectPrefetch = reject;
				})
		);
		void getOrFire(key, prefetchFire).catch(() => {});
		expect(prefetchFire).toHaveBeenCalledTimes(1);
		const foregroundFire = vi.fn(async () => ({ session_id: 'fg' }) as CreateSessionResponse);
		const clicked = getOrFire(key, foregroundFire, () => {});
		rejectPrefetch(new Error('gate refused'));
		await expect(clicked).resolves.toMatchObject({ session_id: 'fg' });
		expect(foregroundFire).toHaveBeenCalledTimes(1);
	});

	it('refires immediately as foreground when attaching to a started prefetch', async () => {
		// A started background prefetch may be sleeping behind the
		// backend gate's Background pacing with its SSE already open. A
		// click attaching to it must not wait for that promise to settle
		// — it aborts the shared background request (keeping it running
		// would double the provider traffic for the same episode) and fires
		// fresh with its own Interactive closure right away.
		const key = makeKey('show', 5, 'sub', 'best');
		let bgAborted = false;
		const prefetchFire = vi.fn(
			(_emit: (p: PlayProgress) => void, signal: AbortSignal) =>
				new Promise<CreateSessionResponse>(() => {
					signal.addEventListener('abort', () => {
						bgAborted = true;
					});
				})
		);
		void getOrFire(key, prefetchFire).catch(() => {});
		expect(prefetchFire).toHaveBeenCalledTimes(1);
		const foregroundFire = vi.fn(async () => ({ session_id: 'fg' }) as CreateSessionResponse);
		const clicked = getOrFire(key, foregroundFire, () => {});
		expect(foregroundFire).toHaveBeenCalledTimes(1);
		expect(bgAborted).toBe(true);
		await expect(clicked).resolves.toMatchObject({ session_id: 'fg' });
	});

	it('reuses a resolved background prefetch instead of refiring', async () => {
		// Once the prefetch has fulfilled there is nothing to wait
		// behind — the cached session IS the payoff of warming. A click
		// must get it instantly, not burn a fresh resolve.
		const key = makeKey('show', 6, 'sub', 'best');
		const prefetchFire = vi.fn(plainFire(fakeResp('warmed')));
		await getOrFire(key, prefetchFire);
		const foregroundFire = vi.fn(async () => ({ session_id: 'fg' }) as CreateSessionResponse);
		const clicked = await getOrFire(key, foregroundFire, () => {});
		expect(clicked.session_id).toBe('warmed');
		expect(foregroundFire).not.toHaveBeenCalled();
	});

	it('a click refired from a prefetch still dies on page unmount', async () => {
		// clearForShow on unmount aborts in-flight fires; the foreground
		// refire is a normal cache entry, so unmount aborts it like any
		// other — the request must not outlive the page, and nothing
		// resurrects it afterwards.
		const key = makeKey('gone', 1, 'sub', 'best');
		const prefetchFire = vi.fn(
			(_emit: (p: PlayProgress) => void, signal: AbortSignal) =>
				new Promise<CreateSessionResponse>((_, reject) => {
					signal.addEventListener('abort', () => reject(new Error('aborted')));
				})
		);
		void getOrFire(key, prefetchFire).catch(() => {});
		const foregroundFire = vi.fn(
			(_emit: (p: PlayProgress) => void, signal: AbortSignal) =>
				new Promise<CreateSessionResponse>((_, reject) => {
					signal.addEventListener('abort', () => reject(new Error('aborted')));
				})
		);
		const clicked = getOrFire(key, foregroundFire, () => {});
		clearForShow('gone');
		await expect(clicked).rejects.toThrow();
		expect(foregroundFire).toHaveBeenCalledTimes(1);
	});

	it('drops failed entries so a retry can fire fresh', async () => {
		const fire = vi
			.fn<(emit: (p: PlayProgress) => void) => Promise<CreateSessionResponse>>()
			.mockRejectedValueOnce(new Error('boom'))
			.mockResolvedValueOnce(fakeResp('ok'));
		const k = makeKey('show-1', 1, 'sub', 'best');

		await expect(getOrFire(k, fire)).rejects.toThrow('boom');
		const recovered = await getOrFire(k, fire);

		expect(recovered.session_id).toBe('ok');
		expect(fire).toHaveBeenCalledTimes(2);
	});

	it('keeps separate entries for different keys', async () => {
		const fireA = vi.fn(plainFire(fakeResp('a')));
		const fireB = vi.fn(plainFire(fakeResp('b')));

		const a = await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fireA);
		const b = await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fireB);

		expect(a.session_id).toBe('a');
		expect(b.session_id).toBe('b');
		expect(fireA).toHaveBeenCalledTimes(1);
		expect(fireB).toHaveBeenCalledTimes(1);
	});

	it('broadcasts progress events to subscribers in arrival order', async () => {
		const k = makeKey('show-1', 1, 'sub', 'best');
		let emitFn: ((p: PlayProgress) => void) | null = null;
		const fire = vi.fn(
			(emit: (p: PlayProgress) => void) =>
				new Promise<CreateSessionResponse>((resolve) => {
					emitFn = emit;
					// Resolves only when test calls emitFn(done) below.
					setTimeout(() => resolve(fakeResp('a')), 0);
				})
		);

		const subA: PlayProgress[] = [];
		const subB: PlayProgress[] = [];
		const promise = getOrFire(k, fire, (p) => subA.push(p));
		// Second subscriber joins via getOrFire (typical: the prefetch
		// fired first with a no-op callback, then a click subscribes).
		getOrFire(k, fire, (p) => subB.push(p));

		expect(emitFn).toBeTruthy();
		emitFn!({ kind: 'links_fetched', provider: 'youtube' });
		emitFn!({ kind: 'links_fetched', provider: 'sharepoint' });

		await promise;
		expect(subA).toEqual([
			{ kind: 'links_fetched', provider: 'youtube' },
			{ kind: 'links_fetched', provider: 'sharepoint' }
		]);
		expect(subB).toEqual(subA);
		// fire only ran once — both subscribers share the underlying stream.
		expect(fire).toHaveBeenCalledTimes(1);
	});

	it('replays the most recent event to a late subscriber', async () => {
		const k = makeKey('show-1', 1, 'sub', 'best');
		let emitFn: ((p: PlayProgress) => void) | null = null;
		const fire = vi.fn(
			(emit: (p: PlayProgress) => void) =>
				new Promise<CreateSessionResponse>((resolve) => {
					emitFn = emit;
					setTimeout(() => resolve(fakeResp('a')), 0);
				})
		);

		// A foreground caller opens the entry. (A *background* entry
		// wouldn't replay: a click attaching to a started prefetch
		// refires fresh with its own stream — see the bypass tests.)
		getOrFire(k, fire, () => {});
		emitFn!({ kind: 'links_fetched', provider: 'youtube' });
		emitFn!({ kind: 'links_fetched', provider: 'sharepoint' });

		// Click subscribes mid-flight: should immediately receive the
		// most recent event, then any future events.
		const lateLog: PlayProgress[] = [];
		const promise = getOrFire(k, fire, (p) => lateLog.push(p));

		expect(lateLog).toEqual([{ kind: 'links_fetched', provider: 'sharepoint' }]);

		emitFn!({ kind: 'links_fetched', provider: 'wixmp' });
		await promise;
		expect(lateLog).toEqual([
			{ kind: 'links_fetched', provider: 'sharepoint' },
			{ kind: 'links_fetched', provider: 'wixmp' }
		]);
	});
});

describe('PREFETCH_CONCURRENCY semaphore', () => {
	it('queues fires beyond the cap and only releases when an in-flight one settles', async () => {
		// PREFETCH_CONCURRENCY = 2 (see prefetch-slots.ts). Fire 4
		// entries at once: the first two should run immediately, the
		// latter two queue for a slot until one frees up.
		let runningNow = 0;
		let peakConcurrent = 0;
		const releasers: Array<() => void> = [];
		const fire = (seed: string) => () =>
			new Promise<CreateSessionResponse>((resolve) => {
				runningNow += 1;
				peakConcurrent = Math.max(peakConcurrent, runningNow);
				releasers.push(() => {
					runningNow -= 1;
					resolve(fakeResp(seed));
				});
			});

		const p1 = getOrFire(makeKey('s', 1, 'sub', 'best'), fire('a'));
		const p2 = getOrFire(makeKey('s', 2, 'sub', 'best'), fire('b'));
		const p3 = getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'));
		const p4 = getOrFire(makeKey('s', 4, 'sub', 'best'), fire('d'));

		// Yield enough microtasks for the first two to enter their fire()
		// bodies; the last two should still be waiting on the queue.
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);
		expect(runningNow).toBe(2);

		// Drain in order. Releasing one slot should immediately admit
		// the next queued fire on the next microtask.
		releasers[0]();
		await p1;
		await Promise.resolve();
		expect(releasers).toHaveLength(3);

		releasers[1]();
		await p2;
		await Promise.resolve();
		expect(releasers).toHaveLength(4);

		releasers[2]();
		releasers[3]();
		await Promise.all([p3, p4]);

		// Cap held at 2 throughout — the queue did its job.
		expect(peakConcurrent).toBe(2);
	});
});

describe('priority subscriber promotion', () => {
	it('cuts a queued entry to the head when a click subscribes', async () => {
		// PREFETCH_CONCURRENCY = 2. Two prefetches saturate the cap; a
		// third queues. A user click on the third (passing an onProgress
		// callback) should promote it to run immediately rather than
		// wait for one of the active two to drain — otherwise the
		// loading overlay sits idle while the resolve queue clears.
		const releasers: Array<() => void> = [];
		const fire = (seed: string) => () =>
			new Promise<CreateSessionResponse>((resolve) => {
				releasers.push(() => resolve(fakeResp(seed)));
			});

		// Two prefetches saturate the cap.
		void getOrFire(makeKey('s', 1, 'sub', 'best'), fire('a'));
		void getOrFire(makeKey('s', 2, 'sub', 'best'), fire('b'));
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);

		// Third prefetch — queues, doesn't run.
		void getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'));
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(2);

		// User clicks ep 3 → attach an onProgress subscriber. The cache
		// should detect this is a click and start the queued fire now.
		void getOrFire(makeKey('s', 3, 'sub', 'best'), fire('c'), () => {});
		await Promise.resolve();
		await Promise.resolve();
		expect(releasers).toHaveLength(3); // ep 3 is running

		// Drain so the test exits cleanly.
		releasers.forEach((r) => r());
	});

	it('is a no-op when the entry has already started running', async () => {
		// A click on an actively-running entry should not double-fire.
		const fire = vi.fn(plainFire(fakeResp('a')));
		const k = makeKey('show', 1, 'sub', 'best');
		const promise = getOrFire(k, fire);
		await promise; // entry now resolved
		void getOrFire(k, fire, () => {}); // subscribe later
		await Promise.resolve();
		expect(fire).toHaveBeenCalledTimes(1);
	});
});

describe('clearForShow — abort in-flight', () => {
	it('aborts the signal passed to fire, propagating cancellation downstream', async () => {
		// fire receives an AbortSignal. clearForShow flips it. The
		// playStream layer (real fire) closes its EventSource on the
		// signal so abandoned prefetches stop streaming.
		let receivedSignal: AbortSignal | null = null;
		const fire = (
			_emit: (p: PlayProgress) => void,
			signal: AbortSignal
		): Promise<CreateSessionResponse> => {
			receivedSignal = signal;
			return new Promise<CreateSessionResponse>((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(new Error('aborted')));
			});
		};

		const k = makeKey('show-x', 1, 'sub', 'best');
		const promise = getOrFire(k, fire);
		await Promise.resolve();
		expect(receivedSignal).not.toBeNull();
		expect(receivedSignal!.aborted).toBe(false);

		clearForShow('show-x');
		expect(receivedSignal!.aborted).toBe(true);
		await expect(promise).rejects.toThrow('aborted');
	});
});

describe('clearForShow — abort queued', () => {
	it('never runs a fire that was cleared while it waited for a slot', async () => {
		// PREFETCH_CONCURRENCY = 2. Two fires hold the slots and a third
		// queues; leaving the show aborts all three. The two running
		// ones reject on their signal and free their slots — the queued
		// one must not then start: its page is gone, and a fire is a
		// resolve request to the backend.
		const started: string[] = [];
		const fire =
			(seed: string) =>
			(_emit: (p: PlayProgress) => void, signal: AbortSignal): Promise<CreateSessionResponse> => {
				started.push(seed);
				return new Promise<CreateSessionResponse>((_resolve, reject) => {
					signal.addEventListener('abort', () => reject(new Error('aborted')));
				});
			};
		const p1 = getOrFire(makeKey('gone', 1, 'sub', 'best'), fire('a'));
		const p2 = getOrFire(makeKey('gone', 2, 'sub', 'best'), fire('b'));
		const p3 = getOrFire(makeKey('gone', 3, 'sub', 'best'), fire('c'));
		await Promise.resolve();
		await Promise.resolve();
		expect(started).toEqual(['a', 'b']);

		clearForShow('gone');
		await expect(p1).rejects.toThrow(/aborted/);
		await expect(p2).rejects.toThrow(/aborted/);
		await Promise.resolve();
		await Promise.resolve();
		expect(started).toEqual(['a', 'b']);
		await expect(p3).rejects.toThrow(/aborted/);
	});

	it('leaves the slots to the fires still queued for another show', async () => {
		// The cleared entry gives up its place; the queue behind it is
		// not held up, and the cap still holds.
		const releasers: Array<() => void> = [];
		const started: string[] = [];
		const fire =
			(seed: string) =>
			(_emit: (p: PlayProgress) => void, signal: AbortSignal): Promise<CreateSessionResponse> => {
				started.push(seed);
				return new Promise<CreateSessionResponse>((resolve, reject) => {
					releasers.push(() => resolve(fakeResp(seed)));
					signal.addEventListener('abort', () => reject(new Error('aborted')));
				});
			};
		const a = getOrFire(makeKey('kept', 1, 'sub', 'best'), fire('a'));
		const b = getOrFire(makeKey('kept', 2, 'sub', 'best'), fire('b'));
		const gone = getOrFire(makeKey('gone', 1, 'sub', 'best'), fire('gone'));
		const c = getOrFire(makeKey('kept', 3, 'sub', 'best'), fire('c'));
		await Promise.resolve();
		await Promise.resolve();
		clearForShow('gone');

		releasers[0]();
		await a;
		await Promise.resolve();
		await Promise.resolve();
		expect(started).toEqual(['a', 'b', 'c']);
		await expect(gone).rejects.toThrow(/aborted/);
		releasers[1]();
		releasers[2]();
		await Promise.all([b, c]);
	});
});

describe('clearBackgroundForShow', () => {
	/** A fire that never answers and reports whether it was aborted. */
	function heldFire() {
		const seen: { signal?: AbortSignal } = {};
		const fire = (_emit: (p: PlayProgress) => void, signal: AbortSignal) => {
			seen.signal = signal;
			return new Promise<CreateSessionResponse>((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(new Error('aborted')));
			});
		};
		return { fire, seen };
	}

	it("aborts the show's warms, running or queued, and forgets them", async () => {
		const running = heldFire();
		const alsoRunning = heldFire();
		const queued = heldFire();
		const a = getOrFire(makeKey('show-1', 1, 'sub', 'best'), running.fire);
		const b = getOrFire(makeKey('show-1', 2, 'sub', 'best'), alsoRunning.fire);
		const c = getOrFire(makeKey('show-1', 3, 'sub', 'best'), queued.fire);
		for (const p of [a, b, c]) p.catch(() => {});
		await Promise.resolve();
		clearBackgroundForShow('show-1');
		expect(running.seen.signal?.aborted).toBe(true);
		expect(alsoRunning.seen.signal?.aborted).toBe(true);
		await new Promise((r) => setTimeout(r, 0));
		// The queued one never ran.
		expect(queued.seen.signal).toBeUndefined();
		const again = vi.fn(() => Promise.resolve(fakeResp('fresh')));
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), again);
		expect(again).toHaveBeenCalledTimes(1);
	});

	it("leaves a click's own play, another show's warms and an answered warm", async () => {
		const click = heldFire();
		const other = heldFire();
		const answered = vi.fn(() => Promise.resolve(fakeResp('done')));
		await getOrFire(makeKey('show-1', 5, 'sub', 'best'), answered);
		const p = getOrFire(makeKey('show-1', 1, 'sub', 'best'), click.fire, () => {});
		const q = getOrFire(makeKey('show-2', 1, 'sub', 'best'), other.fire);
		for (const x of [p, q]) x.catch(() => {});
		await Promise.resolve();
		clearBackgroundForShow('show-1');
		expect(click.seen.signal?.aborted).toBe(false);
		expect(other.seen.signal?.aborted).toBe(false);
		const reuse = vi.fn(() => Promise.resolve(fakeResp('again')));
		expect(await getOrFire(makeKey('show-1', 5, 'sub', 'best'), reuse)).toEqual(fakeResp('done'));
		expect(reuse).not.toHaveBeenCalled();
	});
});

describe('clearBackgroundForOtherVersions', () => {
	/** A fire that never answers and reports whether it was aborted. */
	function heldFire() {
		const seen: { signal?: AbortSignal } = {};
		const fire = (_emit: (p: PlayProgress) => void, signal: AbortSignal) => {
			seen.signal = signal;
			return new Promise<CreateSessionResponse>((_resolve, reject) => {
				signal.addEventListener('abort', () => reject(new Error('aborted')));
			});
		};
		return { fire, seen };
	}

	// A new version choice re-warms the show under the chosen version's
	// keys. The other version's warms are for a listing nobody will
	// play now, and left running they hold the prefetch slots and the
	// resolve lane ahead of the chosen version's.
	it("aborts the show's warms of every other version, running or queued", async () => {
		const unversioned = heldFire();
		const uncensored = heldFire();
		const queued = heldFire();
		const a = getOrFire(makeKey('show-1', 1, 'sub', 'best'), unversioned.fire);
		const b = getOrFire(makeKey('show-1', 2, 'sub', 'best', 'uncensored'), uncensored.fire);
		const c = getOrFire(makeKey('show-1', 3, 'sub', 'best'), queued.fire);
		for (const p of [a, b, c]) p.catch(() => {});
		await Promise.resolve();
		clearBackgroundForOtherVersions('show-1', 'standard');
		expect(unversioned.seen.signal?.aborted).toBe(true);
		expect(uncensored.seen.signal?.aborted).toBe(true);
		await new Promise((r) => setTimeout(r, 0));
		expect(queued.seen.signal).toBeUndefined();
		const again = vi.fn(() => Promise.resolve(fakeResp('fresh')));
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), again);
		expect(again).toHaveBeenCalledTimes(1);
	});

	it("leaves the chosen version's warms, a click's own play and another show's", async () => {
		const chosen = heldFire();
		const click = heldFire();
		const other = heldFire();
		// The two warms take both prefetch slots; the click cuts past.
		const r = getOrFire(makeKey('show-2', 1, 'sub', 'best'), other.fire);
		const p = getOrFire(makeKey('show-1', 1, 'sub', 'best', 'standard'), chosen.fire);
		const q = getOrFire(makeKey('show-1', 2, 'sub', 'best'), click.fire, () => {});
		for (const x of [p, q, r]) x.catch(() => {});
		await Promise.resolve();
		clearBackgroundForOtherVersions('show-1', 'standard');
		expect(chosen.seen.signal?.aborted).toBe(false);
		expect(click.seen.signal?.aborted).toBe(false);
		expect(other.seen.signal?.aborted).toBe(false);
	});

	it('reads no version as the choice of the default copy', async () => {
		const unversioned = heldFire();
		const standard = heldFire();
		const a = getOrFire(makeKey('show-1', 1, 'sub', 'best'), unversioned.fire);
		const b = getOrFire(makeKey('show-1', 2, 'sub', 'best', 'standard'), standard.fire);
		for (const p of [a, b]) p.catch(() => {});
		await Promise.resolve();
		clearBackgroundForOtherVersions('show-1', undefined);
		expect(unversioned.seen.signal?.aborted).toBe(false);
		expect(standard.seen.signal?.aborted).toBe(true);
	});
});

describe('clearForShow', () => {
	it('drops every entry whose key starts with the given show id', async () => {
		const fire = vi.fn(plainFire(fakeResp('x')));
		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-1', 2, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		clearForShow('show-1');

		await getOrFire(makeKey('show-1', 1, 'sub', 'best'), fire);
		await getOrFire(makeKey('show-2', 1, 'sub', 'best'), fire);

		expect(fire).toHaveBeenCalledTimes(4);
	});
});
