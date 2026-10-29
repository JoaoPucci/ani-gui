import { describe, expect, it, vi } from 'vitest';

import { createRailMotionGate, railDelta } from './watch-later-motion';

describe('railDelta', () => {
	it('sees a show join the row, and the cards after it move over', () => {
		expect(railDelta(['a', 'b', 'c'], ['n', 'a', 'b', 'c'])).toEqual({
			added: ['n'],
			removed: [],
			shifted: ['a', 'b', 'c']
		});
		expect(railDelta(['a', 'b', 'c'], ['a', 'n', 'b', 'c'])).toEqual({
			added: ['n'],
			removed: [],
			shifted: ['b', 'c']
		});
	});

	it('sees a show leave the row, and the cards after it close the gap', () => {
		expect(railDelta(['a', 'b', 'c'], ['a', 'c'])).toEqual({
			added: [],
			removed: ['b'],
			shifted: ['c']
		});
	});

	it('sees nothing when the row is the same', () => {
		expect(railDelta(['a', 'b'], ['a', 'b'])).toEqual({ added: [], removed: [], shifted: [] });
	});

	it('sees a card that swapped places as shifted', () => {
		expect(railDelta(['a', 'b'], ['b', 'a'])).toEqual({
			added: [],
			removed: [],
			shifted: ['b', 'a']
		});
	});
});

describe('createRailMotionGate', () => {
	const timers = () => {
		const pending: (() => void)[] = [];
		return {
			deps: {
				setTimeout: (cb: () => void) => {
					pending.push(cb);
					return pending.length;
				},
				clearTimeout: vi.fn()
			},
			elapse: () => pending.splice(0).forEach((cb) => cb())
		};
	};

	it('lets the changed cards move while it is open, and nothing else', () => {
		const t = timers();
		const gate = createRailMotionGate(350, t.deps);
		gate.open({ added: ['n'], removed: ['b'], shifted: ['c'] });
		expect(gate.arrives('n')).toBe(true);
		expect(gate.leaves('b')).toBe(true);
		expect(gate.shifts('c')).toBe(true);
		expect(gate.arrives('c')).toBe(false);
		expect(gate.leaves('n')).toBe(false);
		expect(gate.shifts('a')).toBe(false);
	});

	it('lets a shifted card slide once, as the Continue Watching gate does', () => {
		const t = timers();
		const gate = createRailMotionGate(350, t.deps);
		gate.open({ added: [], removed: [], shifted: ['c'] });
		expect(gate.shifts('c')).toBe(true);
		expect(gate.shifts('c')).toBe(false);
	});

	it('closes once its time is up: a later load moves nothing', () => {
		const t = timers();
		const gate = createRailMotionGate(350, t.deps);
		gate.open({ added: ['n'], removed: ['b'], shifted: ['c'] });
		t.elapse();
		expect(gate.arrives('n')).toBe(false);
		expect(gate.leaves('b')).toBe(false);
		expect(gate.shifts('c')).toBe(false);
	});

	it('moves nothing before it is opened', () => {
		const gate = createRailMotionGate();
		expect(gate.arrives('n')).toBe(false);
		expect(gate.leaves('n')).toBe(false);
		expect(gate.shifts('n')).toBe(false);
	});
});
