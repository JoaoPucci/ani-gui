// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { TipSteps, foundSteps } from './tip-steps.svelte';

const places = (n: number) =>
	Array.from({ length: n }, (_, i) => {
		const el = document.createElement('div');
		el.id = `p${i}`;
		document.body.appendChild(el);
		return el;
	});

describe('TipSteps', () => {
	it('walks its places with Next and Back, and Next on the last is Done', () => {
		const ps = places(3);
		const s = new TipSteps(ps);
		expect(s.place).toBe(ps[0]);
		expect(s.go('back')).toBe('stayed');
		expect(s.go('next')).toBe('moved');
		expect(s.place).toBe(ps[1]);
		expect(s.travelling).toBe(true);
		s.arrived();
		expect(s.travelling).toBe(false);
		expect(s.go('next')).toBe('moved');
		expect(s.go('next')).toBe('done');
		expect(s.place).toBe(ps[2]);
	});

	it('a single place is one step, done at once', () => {
		const s = new TipSteps(places(1));
		expect(s.seq.total).toBe(1);
		expect(s.go('next')).toBe('done');
	});

	it('is connected while every place is on the page', () => {
		const ps = places(2);
		const s = new TipSteps(ps);
		expect(s.connected()).toBe(true);
		ps[1].remove();
		expect(s.connected()).toBe(false);
	});
});

describe('foundSteps', () => {
	it('leaves out a step whose place is not on the page, keeping the others’ numbers', () => {
		const a = document.createElement('div');
		const c = document.createElement('div');
		const found = foundSteps(['a', 'b', 'c'], (p) => (p === 'a' ? a : p === 'c' ? c : null));
		expect(found).toEqual([
			{ el: a, n: 1 },
			{ el: c, n: 3 }
		]);
		const steps = new TipSteps(
			found.map((f) => f.el),
			found.map((f) => f.n)
		);
		expect(steps.number).toBe(1);
		steps.go('next');
		expect(steps.number).toBe(3);
	});
});
