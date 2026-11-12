// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { nextInCycle, tabbablesIn } from './focus-cycle';

describe('nextInCycle', () => {
	it('moves to the next control, and the one before going backwards', () => {
		expect(nextInCycle(['a', 'b', 'c'], 'a', false)).toBe('b');
		expect(nextInCycle(['a', 'b', 'c'], 'c', true)).toBe('b');
	});

	it('wraps round at both ends', () => {
		expect(nextInCycle(['a', 'b', 'c'], 'c', false)).toBe('a');
		expect(nextInCycle(['a', 'b', 'c'], 'a', true)).toBe('c');
	});

	it('enters at the first, or the last going backwards, from outside them', () => {
		expect(nextInCycle(['a', 'b', 'c'], null, false)).toBe('a');
		expect(nextInCycle(['a', 'b', 'c'], 'x', false)).toBe('a');
		expect(nextInCycle(['a', 'b', 'c'], null, true)).toBe('c');
		expect(nextInCycle(['a', 'b', 'c'], 'x', true)).toBe('c');
	});

	it('stays on a lone control', () => {
		expect(nextInCycle(['a'], 'a', false)).toBe('a');
		expect(nextInCycle(['a'], 'a', true)).toBe('a');
	});

	it('has nowhere to go with nothing to focus', () => {
		expect(nextInCycle([], null, false)).toBeNull();
		expect(nextInCycle([], 'x', true)).toBeNull();
	});
});

describe('nextInCycle, by property', () => {
	const ring = fc
		.uniqueArray(fc.integer(), { minLength: 1, maxLength: 12 })
		.chain((items) => fc.tuple(fc.constant(items), fc.nat(items.length - 1)));

	it('always lands on one of the controls', () => {
		fc.assert(
			fc.property(ring, fc.option(fc.integer()), fc.boolean(), ([items], current, back) => {
				expect(items).toContain(nextInCycle(items, current, back));
			})
		);
	});

	it('comes back to where it started after a full lap, either way', () => {
		fc.assert(
			fc.property(ring, fc.boolean(), ([items, at], back) => {
				let cur: number | null = items[at];
				for (let i = 0; i < items.length; i++) cur = nextInCycle(items, cur, back);
				expect(cur).toBe(items[at]);
			})
		);
	});

	it('Shift+Tab undoes Tab', () => {
		fc.assert(
			fc.property(ring, ([items, at]) => {
				const forward = nextInCycle(items, items[at], false);
				expect(nextInCycle(items, forward, true)).toBe(items[at]);
			})
		);
	});
});

describe('tabbablesIn', () => {
	function root(html: string): HTMLElement {
		const el = document.createElement('div');
		el.innerHTML = html;
		return el;
	}
	const ids = (els: HTMLElement[]) => els.map((e) => e.id);

	it('lists what Tab stops at, in document order', () => {
		const r = root(`
			<button id="b1">one</button>
			<a id="a1" href="#x">link</a>
			<input id="i1" />
			<select id="s1"></select>
			<textarea id="t1"></textarea>
			<div id="d1" tabindex="0"></div>
		`);
		expect(ids(tabbablesIn(r))).toEqual(['b1', 'a1', 'i1', 's1', 't1', 'd1']);
	});

	it('passes over what is out of the order, disabled, hidden or inert', () => {
		const r = root(`
			<a id="no-href">not a link</a>
			<input id="hidden-input" type="hidden" />
			<div id="minus" tabindex="-1"></div>
			<button id="disabled" disabled>off</button>
			<div hidden><button id="in-hidden">x</button></div>
			<div inert><button id="in-inert">x</button></div>
			<button id="kept">kept</button>
		`);
		expect(ids(tabbablesIn(r))).toEqual(['kept']);
	});
});
