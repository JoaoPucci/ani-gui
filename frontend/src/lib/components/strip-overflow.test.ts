import { describe, expect, it } from 'vitest';
import { scrollEdges, watchStripOverflow, type ScrollEdges } from './strip-overflow';

describe('scrollEdges', () => {
	it('offers neither arrow when the content fits the viewport', () => {
		expect(scrollEdges({ scrollLeft: 0, clientWidth: 800, scrollWidth: 600 })).toEqual({
			canScrollLeft: false,
			canScrollRight: false
		});
	});

	it('offers the right arrow at the start of an overflowing row', () => {
		expect(scrollEdges({ scrollLeft: 0, clientWidth: 800, scrollWidth: 1600 })).toEqual({
			canScrollLeft: false,
			canScrollRight: true
		});
	});

	it('offers the left arrow at the end of an overflowing row', () => {
		expect(scrollEdges({ scrollLeft: 800, clientWidth: 800, scrollWidth: 1600 })).toEqual({
			canScrollLeft: true,
			canScrollRight: false
		});
	});

	it('ignores the few pixels of slack sub-pixel layout leaves at either end', () => {
		expect(scrollEdges({ scrollLeft: 4, clientWidth: 800, scrollWidth: 808 })).toEqual({
			canScrollLeft: false,
			canScrollRight: false
		});
	});
});

/** A scroller with settable metrics: no layout engine runs here, so
 *  the test plays the part of one. */
class FakeScroller extends EventTarget {
	scrollLeft = 0;
	clientWidth = 800;
	scrollWidth = 800;
}

/** A rail whose children the test adds and removes, as a keyed
 *  `{#each}` does. */
class FakeRail {
	children: object[] = [];
}

/** Observers that fire only when the test says so — the way a real one
 *  fires only when the thing it watches changes. */
function fakeObservers() {
	const resize: { cb: () => void; observed: unknown[] }[] = [];
	const mutation: { cb: () => void; observed: unknown[] }[] = [];
	class RO {
		observed: unknown[] = [];
		constructor(cb: () => void) {
			resize.push({ cb, observed: this.observed });
		}
		observe(el: unknown) {
			this.observed.push(el);
		}
		unobserve() {}
		disconnect() {
			this.observed.length = 0;
		}
	}
	class MO {
		observed: unknown[] = [];
		constructor(cb: () => void) {
			mutation.push({ cb, observed: this.observed });
		}
		observe(el: unknown) {
			this.observed.push(el);
		}
		disconnect() {
			this.observed.length = 0;
		}
	}
	return {
		observers: { ResizeObserver: RO, MutationObserver: MO },
		/** Fire every resize observer watching `el`. */
		resized(el: unknown) {
			for (const r of resize) if (r.observed.includes(el)) r.cb();
		},
		/** Fire every mutation observer watching `el`. */
		mutated(el: unknown) {
			for (const r of mutation) if (r.observed.includes(el)) r.cb();
		}
	};
}

function watch(scroller: FakeScroller, rail: FakeRail, observers: unknown) {
	const seen: ScrollEdges[] = [];
	const stop = watchStripOverflow(
		scroller as unknown as HTMLElement,
		rail as unknown as HTMLElement,
		(e) => seen.push(e),
		observers as Parameters<typeof watchStripOverflow>[3]
	);
	return { seen, last: () => seen[seen.length - 1], stop };
}

describe('watchStripOverflow', () => {
	it('reports the edges as soon as it starts', () => {
		const scroller = new FakeScroller();
		scroller.scrollWidth = 1600;
		const w = watch(scroller, new FakeRail(), fakeObservers().observers);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: true });
	});

	it('follows a scroll', () => {
		const scroller = new FakeScroller();
		scroller.scrollWidth = 1600;
		const w = watch(scroller, new FakeRail(), fakeObservers().observers);
		scroller.scrollLeft = 800;
		scroller.dispatchEvent(new Event('scroll'));
		expect(w.last()).toEqual({ canScrollLeft: true, canScrollRight: false });
	});

	it('follows the viewport widening past the content', () => {
		const scroller = new FakeScroller();
		scroller.scrollWidth = 1000;
		const fake = fakeObservers();
		const w = watch(scroller, new FakeRail(), fake.observers);
		scroller.clientWidth = 1200;
		scroller.scrollWidth = 1200;
		fake.resized(scroller);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: false });
	});

	it('drops the right arrow when the cards shrink inside an unchanged viewport', () => {
		// Continue Watching renders a card per history row, then
		// collapses rows that resolve to one show; a deleted card or a
		// cleared history shrinks it the same way. The scroller's own
		// box keeps its size throughout, so only the rail's change can
		// tell the strip its content no longer overflows.
		const scroller = new FakeScroller();
		const rail = new FakeRail();
		rail.children = [{}, {}, {}, {}, {}];
		scroller.scrollWidth = 1400;
		const fake = fakeObservers();
		const w = watch(scroller, rail, fake.observers);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: true });

		rail.children = [{}, {}];
		scroller.scrollWidth = 800;
		fake.mutated(rail);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: false });
	});

	it('offers the right arrow when cards arrive that overflow the viewport', () => {
		const scroller = new FakeScroller();
		const rail = new FakeRail();
		const fake = fakeObservers();
		const w = watch(scroller, rail, fake.observers);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: false });

		rail.children = [{}, {}, {}, {}, {}];
		scroller.scrollWidth = 1400;
		fake.mutated(rail);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: true });
	});

	it('follows a card changing size without the card list changing', () => {
		const scroller = new FakeScroller();
		const card = {};
		const rail = new FakeRail();
		rail.children = [card];
		scroller.scrollWidth = 1400;
		const fake = fakeObservers();
		const w = watch(scroller, rail, fake.observers);

		scroller.scrollWidth = 600;
		fake.resized(card);
		expect(w.last()).toEqual({ canScrollLeft: false, canScrollRight: false });
	});

	it('stops reporting once torn down', () => {
		const scroller = new FakeScroller();
		const w = watch(scroller, new FakeRail(), fakeObservers().observers);
		w.stop();
		const count = w.seen.length;
		scroller.dispatchEvent(new Event('scroll'));
		expect(w.seen.length).toBe(count);
	});
});
