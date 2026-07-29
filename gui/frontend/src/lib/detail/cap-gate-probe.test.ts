import { describe, it, expect } from 'vitest';

import { createCapGateProbe, type CapGateAnswer, type CapGateRefresh } from './cap-gate-probe';

/** Drain the microtask queue. Counting `await Promise.resolve()`
 *  ticks is brittle — the settle path runs through then/catch/finally
 *  and the depth is an implementation detail, not the behaviour. */
const flush = () => new Promise<void>((r) => setTimeout(r, 0));

/** A probe whose resolution the test controls. */
function deferredProbe() {
	let release!: (answer: CapGateAnswer | null) => void;
	let reject!: (e: unknown) => void;
	const calls: number[] = [];
	const probe = () => {
		calls.push(calls.length + 1);
		return new Promise<CapGateAnswer | null>((res, rej) => {
			release = res;
			reject = rej;
		});
	};
	return {
		calls,
		probe,
		// The catalogue answered — possibly without a number. Distinct
		// from `fail()`, which is not being able to ask at all. The
		// common case is a show that is there with no specials, so
		// that is the default and cases override what they are about.
		release: (count: number | null, approximate = false, rest: Partial<CapGateAnswer> = {}) =>
			release({ count, approximate, available: true, extraEpisodes: [], ...rest }),
		fail: () => reject(new Error('probe failed'))
	};
}

function harness(probe: () => Promise<CapGateAnswer | null>) {
	const cleared: { episode: number; count: number }[] = [];
	const stillGated: number[] = [];
	const failed: number[] = [];
	const failedRefreshes: (CapGateRefresh | null)[] = [];
	const stillGatedCounts: (number | null)[] = [];
	const superseded: number[] = [];
	/** Every refresh the controller judged safe to write, in order,
	 *  from whichever outcome delivered it. */
	const refreshes: CapGateRefresh[] = [];
	/** What the answer will be about: the show the strip is on AND the
	 *  audio mode. Both routes reuse one component across shows, and
	 *  the mode arrives asynchronously from settings — so either can
	 *  move under an in-flight lookup. */
	const showing = { id: 'show-a', mode: 'sub' };
	const gate = createCapGateProbe({
		probe,
		currentContext: () => `${showing.id}:${showing.mode}`,
		onCleared: (episode, count, refresh) => {
			cleared.push({ episode, count });
			refreshes.push(refresh);
		},
		onStillGated: (episode, refresh) => {
			stillGated.push(episode);
			stillGatedCounts.push(refresh.count);
			refreshes.push(refresh);
		},
		onFailed: (episode, refresh) => {
			failed.push(episode);
			failedRefreshes.push(refresh);
		},
		onSuperseded: (episode) => superseded.push(episode)
	});
	return {
		gate,
		cleared,
		stillGated,
		stillGatedCounts,
		refreshes,
		failed,
		failedRefreshes,
		superseded,
		showing
	};
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
		const { gate, cleared, stillGated, failed } = harness(d.probe);

		gate.request(5);
		// Reaches the episode, but came from the search hit rather than
		// the per-show fetch: it counts half-episodes as whole ones and
		// can read one high. Clearing on it starts resolving an episode
		// that does not exist.
		d.release(7, true);
		await flush();

		expect(cleared).toEqual([]);
		// And it is not a catalogue verdict either. The per-show fetch
		// is what would have said whether this episode is there, and it
		// is the thing that did not answer — so "still not in the
		// catalogue" asserts exactly what went unestablished.
		expect(stillGated).toEqual([]);
		expect(failed).toEqual([5]);
	});

	it('has nothing to hand back when the lookup could not be made', async () => {
		const d = deferredProbe();
		const { gate, failed, failedRefreshes } = harness(d.probe);

		gate.request(5);
		d.fail();
		await flush();

		// Offline or rate-limited: no answer at all, so nothing about
		// the show was established. Distinct from the case above, where
		// half of one arrived.
		expect(failed).toEqual([5]);
		expect(failedRefreshes).toEqual([null]);
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

	it('carries the catalogue verdict when the show is no longer listed', async () => {
		const d = deferredProbe();
		const { gate, refreshes } = harness(d.probe);

		gate.request(5);
		// allmanga delisted the show, or the resolver corrected which
		// title it had matched. A false verdict is always definite —
		// a search that could not be completed is an error, not a no.
		d.release(null, false, { available: false });
		await flush();

		// Without this the page keeps the availability it was told at
		// mount, so every episode stays enabled and every click starts
		// a resolution against a show that is not there.
		//
		// The cap comes back as 0 rather than null. Null is the routes'
		// word for "no cap known", which `beyondPlayable` reads as
		// UNBOUNDED — the opposite of what a delisted show means. A
		// confirmed absence is a cap of zero.
		expect(refreshes).toEqual([{ available: false, count: 0, extraEpisodes: [] }]);
	});

	it('caps at zero when the catalogue confirms it has no whole episodes', async () => {
		const d = deferredProbe();
		const { gate, refreshes } = harness(d.probe);

		gate.request(5);
		// The show is listed and the per-show fetch answered — it just
		// has no integer episode tags. That is a fact about the
		// catalogue, not a gap in what we learned.
		d.release(null, false, { available: true, extraEpisodes: ['1.5'] });
		await flush();

		// Leaving the page's previous cap in place would keep every
		// tile under it playable against a row that now says none of
		// them exist.
		expect(refreshes).toEqual([{ available: true, count: 0, extraEpisodes: ['1.5'] }]);
	});

	it('carries refreshed extras so the strip picks up specials', async () => {
		const d = deferredProbe();
		const { gate, refreshes } = harness(d.probe);

		gate.request(9);
		d.release(6, false, { extraEpisodes: ['4.5'] });
		await flush();

		// The strip splices non-integer tags in at their numeric
		// position. A special catalogued since the page loaded is
		// invisible until this list is replaced — and one that was
		// pulled stays on screen and playable.
		expect(refreshes).toEqual([{ available: true, count: 6, extraEpisodes: ['4.5'] }]);
	});

	it('withholds count and extras from an unconfirmed answer, but keeps the verdict', async () => {
		const d = deferredProbe();
		const { gate, failedRefreshes } = harness(d.probe);

		gate.request(9);
		// Unconfirmed means the per-show fetch failed — and that fetch
		// is what supplies both the exact count and the specials. The
		// count can read high; the empty list is "we could not look",
		// not "there are none", so writing it would delete specials
		// that exist.
		//
		// The verdict survives, because the SEARCH is what produced it
		// and the search answered. A page showing the show as
		// unavailable can recover from that.
		d.release(6, true, { extraEpisodes: [] });
		await flush();

		expect(failedRefreshes).toEqual([{ available: true, count: null, extraEpisodes: null }]);
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
