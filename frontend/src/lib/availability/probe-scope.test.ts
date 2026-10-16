import { describe, expect, it } from 'vitest';
import { createProbeScope } from './probe-scope';

describe('createProbeScope', () => {
	it('probes asked on one page share its signal', () => {
		let path = '/';
		const scope = createProbeScope(() => path);
		expect(scope.signal()).toBe(scope.signal());
		path = '/search';
		expect(scope.signal()).not.toBe(undefined);
	});

	it("leaving a page aborts its probes once the next page is up, and not the next page's", () => {
		let path = '/';
		const scope = createProbeScope(() => path);
		const home = scope.signal();
		// The next page mounts and asks before the navigation settles.
		path = '/anime/1';
		const detail = scope.signal();
		scope.settle();
		expect(home.aborted).toBe(true);
		expect(detail.aborted).toBe(false);
	});

	it('a navigation that keeps the path (a query change) aborts nothing', () => {
		const scope = createProbeScope(() => '/anime/1');
		const probe = scope.signal();
		scope.settle();
		expect(probe.aborted).toBe(false);
	});

	it('coming back to a page asks under a fresh signal', () => {
		let path = '/';
		const scope = createProbeScope(() => path);
		const first = scope.signal();
		path = '/search';
		scope.settle();
		path = '/';
		scope.settle();
		const again = scope.signal();
		expect(first.aborted).toBe(true);
		expect(again.aborted).toBe(false);
		expect(again).not.toBe(first);
	});
});
