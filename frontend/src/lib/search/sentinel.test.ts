import { describe, expect, it, vi } from 'vitest';
import { SENTINEL_MARGIN, watchSentinel } from './sentinel';

class FakeObserver {
	static last: FakeObserver | null = null;
	observed: Element[] = [];
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
		const stop = watchSentinel({} as Element, () => {}, FakeObserver);
		stop();
		expect(FakeObserver.last!.disconnected).toBe(true);
	});

	it('without an observer the marker is never in reach', () => {
		const onChange = vi.fn();
		const stop = watchSentinel({} as Element, onChange, undefined);
		expect(onChange).toHaveBeenCalledWith(false);
		expect(() => stop()).not.toThrow();
	});
});
