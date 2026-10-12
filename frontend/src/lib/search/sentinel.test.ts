import { describe, expect, it, vi } from 'vitest';
import { SENTINEL_MARGIN, watchSentinel } from './sentinel';

class FakeObserver {
	static last: FakeObserver | null = null;
	observed: Element[] = [];
	unobserved: Element[] = [];
	disconnected = false;
	constructor(
		public cb: IntersectionObserverCallback,
		public init?: IntersectionObserverInit
	) {
		FakeObserver.last = this;
	}
	observe(el: Element) {
		this.observed.push(el);
	}
	unobserve(el: Element) {
		this.unobserved.push(el);
	}
	disconnect() {
		this.disconnected = true;
	}
	fire(...states: boolean[]) {
		this.cb(
			states.map((isIntersecting) => ({ isIntersecting }) as IntersectionObserverEntry),
			this as unknown as IntersectionObserver
		);
	}
}

describe('watchSentinel', () => {
	it('observes the marker against the viewport extended downward', () => {
		const node = {} as Element;
		watchSentinel(node, () => {}, FakeObserver);
		const o = FakeObserver.last!;
		expect(o.observed).toEqual([node]);
		expect(o.init?.root ?? null).toBeNull();
		expect(o.init?.rootMargin).toBe(SENTINEL_MARGIN);
	});

	it('reports the latest entry of each batch', () => {
		const onChange = vi.fn();
		watchSentinel({} as Element, onChange, FakeObserver);
		FakeObserver.last!.fire(false, true);
		FakeObserver.last!.fire(false);
		expect(onChange.mock.calls).toEqual([[true], [false]]);
	});

	it('stops watching when told to', () => {
		const { stop } = watchSentinel({} as Element, () => {}, FakeObserver);
		stop();
		expect(FakeObserver.last!.disconnected).toBe(true);
	});

	it('without an observer the marker is never in reach', () => {
		const onChange = vi.fn();
		const watch = watchSentinel({} as Element, onChange, undefined);
		expect(onChange).toHaveBeenCalledWith(false);
		expect(() => watch.stop()).not.toThrow();
		expect(() => watch.remeasure()).not.toThrow();
	});

	it('remeasuring observes the marker afresh, so the observer reports its state now', () => {
		// An observer reports a target's state when it starts observing
		// it, changed or not; unobserve then observe is how to ask.
		const node = {} as Element;
		const watch = watchSentinel(node, () => {}, FakeObserver);
		const o = FakeObserver.last!;
		watch.remeasure();
		expect(o.unobserved).toEqual([node]);
		expect(o.observed).toEqual([node, node]);
	});
});
