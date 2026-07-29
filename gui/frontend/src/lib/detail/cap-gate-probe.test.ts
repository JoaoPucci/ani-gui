import { describe, it, expect } from 'vitest';

import { createCapGateProbe } from './cap-gate-probe';

/** Drain the microtask queue. Counting `await Promise.resolve()`
 *  ticks is brittle — the settle path runs through then/catch/finally
 *  and the depth is an implementation detail, not the behaviour. */
const flush = () => new Promise<void>((r) => setTimeout(r, 0));

/** A probe whose resolution the test controls. */
function deferredProbe() {
	let release!: (count: number | null) => void;
	let reject!: (e: unknown) => void;
	const calls: number[] = [];
	const probe = (episode: number) => {
		calls.push(episode);
		return new Promise<number | null>((res, rej) => {
			release = res;
			reject = rej;
		});
	};
	return {
		calls,
		probe,
		release: (count: number | null) => release(count),
		fail: () => reject(new Error('probe failed'))
	};
}

function harness(probe: (episode: number) => Promise<number | null>) {
	const cleared: { episode: number; count: number }[] = [];
	const stillGated: number[] = [];
	const gate = createCapGateProbe({
		probe,
		onCleared: (episode, count) => cleared.push({ episode, count }),
		onStillGated: (episode) => stillGated.push(episode)
	});
	return { gate, cleared, stillGated };
}

describe('createCapGateProbe', () => {
	it('plays the episode when the fresh count now reaches it', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		d.release(7);
		await flush();

		// The click already said the user wants to watch it; the stale
		// cap was the only obstacle, and it is gone.
		expect(cleared).toEqual([{ episode: 5, count: 7 }]);
		expect(stillGated).toEqual([]);
	});

	it('reports still-gated when the fresh count is exactly one short', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		d.release(4);
		await flush();

		expect(cleared).toEqual([]);
		expect(stillGated).toEqual([5]);
	});

	it('clears on a count that lands exactly on the episode', async () => {
		// The boundary the whole gate turns on: a playable count of 5
		// means episode 5 IS streamable.
		const d = deferredProbe();
		const { gate, cleared } = harness(d.probe);

		gate.request(5);
		d.release(5);
		await flush();

		expect(cleared).toEqual([{ episode: 5, count: 5 }]);
	});

	it('reports still-gated when the probe cannot answer', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		d.release(null);
		await flush();

		// A countless answer is not permission to play — it is the
		// absence of one.
		expect(cleared).toEqual([]);
		expect(stillGated).toEqual([5]);
	});

	it('reports still-gated when the probe rejects', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		d.fail();
		await flush();

		// Silence is what makes the current tile feel broken; a failed
		// re-ask still has to say something.
		expect(cleared).toEqual([]);
		expect(stillGated).toEqual([5]);
	});

	it('ignores repeat clicks on an episode already being re-probed', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		gate.request(5);
		gate.request(5);

		// An impatient user must not turn one dimmed tile into a burst
		// of interactive scraper traffic.
		expect(d.calls).toEqual([5]);
		expect(gate.isProbing(5)).toBe(true);
	});

	it('allows a fresh attempt once the previous one settled', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		d.release(4);
		await flush();
		expect(gate.isProbing(5)).toBe(false);

		gate.request(5);
		expect(d.calls).toEqual([5, 5]);
	});

	it('tracks in-flight state per episode', () => {
		const gate = createCapGateProbe({
			probe: () => new Promise<number | null>(() => {}),
			onCleared: () => {},
			onStillGated: () => {}
		});

		gate.request(5);

		// Dimming one tile must not dim its neighbours.
		expect(gate.isProbing(5)).toBe(true);
		expect(gate.isProbing(6)).toBe(false);
	});

	it('releases the in-flight marker even when the probe rejects', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		d.fail();
		await flush();

		// A rejection that left the marker set would wedge the tile
		// permanently: every later click would be swallowed as a
		// duplicate.
		expect(gate.isProbing(5)).toBe(false);
	});

	it('asks about the episode that was clicked', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);
		gate.request(11);
		expect(d.calls).toEqual([11]);
	});
});
