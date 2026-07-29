import { describe, it, expect } from 'vitest';

import { createCapGateProbe } from './cap-gate-probe';

/** Drain the microtask queue. Counting `await Promise.resolve()`
 *  ticks is brittle — the settle path runs through then/catch/finally
 *  and the depth is an implementation detail, not the behaviour. */
const flush = () => new Promise<void>((r) => setTimeout(r, 0));

/** A probe whose resolution the test controls. */
function deferredProbe() {
	let release!: (answer: { count: number | null; approximate: boolean } | null) => void;
	let reject!: (e: unknown) => void;
	const calls: number[] = [];
	const probe = () => {
		calls.push(calls.length + 1);
		return new Promise<{ count: number | null; approximate: boolean } | null>((res, rej) => {
			release = res;
			reject = rej;
		});
	};
	return {
		calls,
		probe,
		// The catalogue answered — possibly without a number. Distinct
		// from `fail()`, which is not being able to ask at all.
		release: (count: number | null, approximate = false) => release({ count, approximate }),
		fail: () => reject(new Error('probe failed'))
	};
}

function harness(probe: () => Promise<{ count: number | null; approximate: boolean } | null>) {
	const cleared: { episode: number; count: number }[] = [];
	const stillGated: number[] = [];
	const failed: number[] = [];
	const stillGatedCounts: (number | null)[] = [];
	const superseded: number[] = [];
	/** What the answer will be about: the show the strip is on AND the
	 *  audio mode. Both routes reuse one component across shows, and
	 *  the mode arrives asynchronously from settings — so either can
	 *  move under an in-flight lookup. */
	const showing = { id: 'show-a', mode: 'sub' };
	const gate = createCapGateProbe({
		probe,
		currentContext: () => `${showing.id}:${showing.mode}`,
		onCleared: (episode, count) => cleared.push({ episode, count }),
		onStillGated: (episode, count) => {
			stillGated.push(episode);
			stillGatedCounts.push(count);
		},
		onFailed: (episode) => failed.push(episode),
		onSuperseded: (episode) => superseded.push(episode)
	});
	return { gate, cleared, stillGated, stillGatedCounts, failed, superseded, showing };
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

	it('refuses to clear on a count the backend could not confirm', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		// Reaches the episode, but came from the search hit rather than
		// the per-show fetch: it counts half-episodes as whole ones and
		// can read one high. Clearing on it starts resolving an episode
		// that does not exist.
		d.release(7, true);
		await flush();

		expect(cleared).toEqual([]);
		expect(stillGated).toEqual([5]);
	});

	it('separates a failed lookup from a confirmed short count', async () => {
		const d = deferredProbe();
		const { gate, stillGated, failed } = harness(d.probe);

		gate.request(5);
		d.fail();
		await flush();

		// Offline, rate-limited or a backend error means we could not
		// ask — saying "still not in the catalogue" claims a fact
		// nobody established.
		expect(stillGated).toEqual([]);
		expect(failed).toEqual([5]);
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

	it('says nothing was learned when the probe rejects', async () => {
		const d = deferredProbe();
		const { gate, cleared, failed } = harness(d.probe);

		gate.request(5);
		d.fail();
		await flush();

		// Silence is what makes the current tile feel broken; a failed
		// re-ask still has to say something — just not something about
		// the catalogue.
		expect(cleared).toEqual([]);
		expect(failed).toEqual([5]);
	});

	it('does not play the old show when the answer lands after a move', async () => {
		const d = deferredProbe();
		const { gate, cleared, superseded, showing } = harness(d.probe);

		gate.request(5);
		// Both routes reuse one component across shows: the user leaves
		// for another title while the lookup is out, and the strip on
		// screen is now a different show's.
		showing.id = 'show-b';
		d.release(7);
		await flush();

		// Playing episode 5 here would start the WRONG show's episode 5
		// — the count belongs to the title the user walked away from.
		expect(cleared).toEqual([]);
		expect(superseded).toEqual([5]);
	});

	it('says nothing about the catalogue when the strip moved on', async () => {
		const d = deferredProbe();
		const { gate, stillGated, failed, superseded, showing } = harness(d.probe);

		gate.request(5);
		showing.id = 'show-b';
		d.release(4);
		await flush();

		// "Still not in the catalogue" would be a claim about a show the
		// user is no longer looking at, attached to an episode number
		// that means something else on this screen. The page still has
		// to hear back, though, or it stays blocked forever.
		expect(stillGated).toEqual([]);
		expect(failed).toEqual([]);
		expect(superseded).toEqual([5]);
	});

	it('does not apply a sub answer once the mode has settled to dub', async () => {
		const d = deferredProbe();
		const { gate, cleared, superseded, showing } = harness(d.probe);

		gate.request(5);
		// Settings arrive after the page does, so a click landing in
		// that gap asks with the fallback mode and the real one turns
		// up while the answer is out.
		showing.mode = 'dub';
		d.release(7);
		await flush();

		// allmanga catalogues sub and dub separately and dub lags, so a
		// sub count of 7 unlocking dub episode 5 resolves something the
		// dub catalogue does not have. Same show, different question.
		expect(cleared).toEqual([]);
		expect(superseded).toEqual([5]);
	});

	it('hands back a confirmed short count so the strip can correct itself', async () => {
		const d = deferredProbe();
		const { gate, stillGated, stillGatedCounts } = harness(d.probe);

		gate.request(9);
		// LOWER than what the page is showing — allmanga pulled an
		// episode, or corrected its metadata. The tiles between the two
		// caps are enabled on a number that is no longer true, and
		// clicking one resolves nothing.
		d.release(6);
		await flush();

		expect(stillGated).toEqual([9]);
		expect(stillGatedCounts).toEqual([6]);
	});

	it('withholds an unconfirmed count from the correction', async () => {
		const d = deferredProbe();
		const { gate, stillGated, stillGatedCounts } = harness(d.probe);

		gate.request(9);
		d.release(6, true);
		await flush();

		// Same shape of answer, but this one came from the search hit
		// and can read high. Publishing it would replace a real cap
		// with a guess.
		expect(stillGated).toEqual([9]);
		expect(stillGatedCounts).toEqual([null]);
	});

	it('still answers normally when the strip stayed put', async () => {
		const d = deferredProbe();
		const { gate, cleared, superseded } = harness(d.probe);

		gate.request(5);
		d.release(7);
		await flush();

		expect(cleared).toEqual([{ episode: 5, count: 7 }]);
		expect(superseded).toEqual([]);
	});

	it('asks once for the whole show, however many tiles are clicked', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		gate.request(5);
		gate.request(6);
		gate.request(7);

		// "How many episodes do you have?" does not vary by episode.
		// Clicking three different dimmed tiles is the same question
		// three times, and the site is rate-limited, so it goes out
		// once and every waiting tile reads the same answer.
		expect(d.calls).toHaveLength(1);
		expect(gate.isProbing(5)).toBe(true);
		expect(gate.isProbing(6)).toBe(true);
		expect(gate.isProbing(7)).toBe(true);
	});

	it('judges every waiting tile against the single answer', async () => {
		const d = deferredProbe();
		const { gate, cleared, stillGated } = harness(d.probe);

		gate.request(5);
		gate.request(9);
		d.release(6);
		await flush();

		// One answer, two verdicts: 5 is within 6 and plays, 9 is not.
		expect(cleared).toEqual([{ episode: 5, count: 6 }]);
		expect(stillGated).toEqual([9]);
	});

	it('allows a fresh attempt once the previous one settled', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		d.release(4);
		await flush();
		expect(gate.isProbing(5)).toBe(false);

		gate.request(5);
		expect(d.calls).toHaveLength(2);
	});

	it('marks only the tiles that were actually clicked as busy', () => {
		const gate = createCapGateProbe({
			probe: () => new Promise<never>(() => {}),
			currentContext: () => 'show-a:sub',
			onCleared: () => {},
			onStillGated: () => {},
			onFailed: () => {},
			onSuperseded: () => {}
		});

		gate.request(5);

		// One lookup covers the show, but the spinner belongs to the
		// tile the user pressed — not to every dimmed tile on screen.
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

	it('asks again for a later click once the first answer landed', async () => {
		const d = deferredProbe();
		const { gate } = harness(d.probe);

		gate.request(5);
		d.release(4);
		await flush();

		gate.request(9);
		// The shared request is only shared while it is in flight; a
		// click after it settled deserves a current answer.
		expect(d.calls).toHaveLength(2);
	});
});
