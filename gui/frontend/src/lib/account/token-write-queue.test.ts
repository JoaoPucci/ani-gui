import { afterEach, describe, expect, it } from 'vitest';
import { __resetTokenWriteQueues, enqueueTokenWrite } from './token-write-queue';

afterEach(() => __resetTokenWriteQueues());

/** A deferred promise we can settle by hand, to control op timing. */
function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

describe('enqueueTokenWrite', () => {
	it('runs two writes for the same provider strictly in order (second waits for the first)', async () => {
		const order: string[] = [];
		const first = deferred<void>();

		const p1 = enqueueTokenWrite('mal', async () => {
			order.push('first-start');
			await first.promise;
			order.push('first-end');
		});
		const p2 = enqueueTokenWrite('mal', async () => {
			order.push('second-start');
		});

		// Let microtasks flush: the second op must NOT have started while
		// the first is still pending.
		await Promise.resolve();
		expect(order).toEqual(['first-start']);

		first.resolve();
		await Promise.all([p1, p2]);
		expect(order).toEqual(['first-start', 'first-end', 'second-start']);
	});

	it('does not serialize across different providers', async () => {
		const order: string[] = [];
		const malGate = deferred<void>();

		const pMal = enqueueTokenWrite('mal', async () => {
			order.push('mal-start');
			await malGate.promise;
		});
		const pAnilist = enqueueTokenWrite('anilist', async () => {
			order.push('anilist-start');
		});

		// AniList must run even though the MAL op is still blocked.
		await pAnilist;
		expect(order).toEqual(['mal-start', 'anilist-start']);

		malGate.resolve();
		await pMal;
	});

	it('a rejecting write does not poison the chain — the next op still runs', async () => {
		const ran: string[] = [];
		const p1 = enqueueTokenWrite('mal', async () => {
			ran.push('one');
			throw new Error('boom');
		});
		await expect(p1).rejects.toThrow('boom');

		const p2 = enqueueTokenWrite('mal', async () => {
			ran.push('two');
			return 42;
		});
		await expect(p2).resolves.toBe(42);
		expect(ran).toEqual(['one', 'two']);
	});

	it('returns the op result to the caller', async () => {
		await expect(enqueueTokenWrite('inhouse', async () => 'ok')).resolves.toBe('ok');
	});
});
