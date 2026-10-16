import { afterEach, describe, expect, it, vi } from 'vitest';
import { handleRadioKey, radioKeyTarget, type RadioKeyEvent } from './radio-keys';

describe('radioKeyTarget', () => {
	it('moves forward on the down and inline-forward arrows, wrapping at the end', () => {
		expect(radioKeyTarget('ArrowDown', 0, 2)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 0, 2)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 1, 2)).toBe(0);
	});

	it('moves back on the up and inline-backward arrows, wrapping at the start', () => {
		expect(radioKeyTarget('ArrowUp', 1, 2)).toBe(0);
		expect(radioKeyTarget('ArrowLeft', 1, 2)).toBe(0);
		expect(radioKeyTarget('ArrowLeft', 0, 3)).toBe(2);
	});

	it('swaps the horizontal arrows in a right-to-left layout, and only those', () => {
		expect(radioKeyTarget('ArrowLeft', 0, 3, true)).toBe(1);
		expect(radioKeyTarget('ArrowRight', 0, 3, true)).toBe(2);
		expect(radioKeyTarget('ArrowDown', 0, 3, true)).toBe(1);
	});

	it('jumps to the ends on Home and End', () => {
		expect(radioKeyTarget('Home', 2, 3)).toBe(0);
		expect(radioKeyTarget('End', 0, 3)).toBe(2);
	});

	it('answers nothing for any other key, or an empty group', () => {
		expect(radioKeyTarget('Enter', 0, 2)).toBeNull();
		expect(radioKeyTarget('a', 0, 2)).toBeNull();
		expect(radioKeyTarget('ArrowDown', 0, 0)).toBeNull();
	});
});

describe('handleRadioKey', () => {
	afterEach(() => vi.unstubAllGlobals());

	/** A group of `count` radios as the handler sees them: each one's
	 *  siblings, in order, with focus recorded. */
	function group(count: number) {
		const focused: number[] = [];
		const radios = Array.from({ length: count }, (_, i) => ({ focus: () => focused.push(i) }));
		const parent = { children: radios };
		for (const r of radios) Object.assign(r, { parentElement: parent });
		const press = (key: string, index: number) => {
			const e = {
				key,
				currentTarget: radios[index] as unknown as EventTarget,
				preventDefault: vi.fn()
			} satisfies RadioKeyEvent;
			return e;
		};
		return { focused, press };
	}

	it('chooses and focuses the radio the key moves to, and keeps the key from scrolling', () => {
		const g = group(2);
		const chosen: number[] = [];
		const e = g.press('ArrowRight', 0);
		handleRadioKey(
			e,
			0,
			2,
			(i) => chosen.push(i),
			() => false
		);
		expect(chosen).toEqual([1]);
		expect(g.focused).toEqual([1]);
		expect(e.preventDefault).toHaveBeenCalledOnce();
	});

	it('leaves any other key alone', () => {
		const g = group(2);
		const choose = vi.fn();
		const e = g.press('Tab', 0);
		handleRadioKey(e, 0, 2, choose, () => false);
		expect(choose).not.toHaveBeenCalled();
		expect(e.preventDefault).not.toHaveBeenCalled();
		expect(g.focused).toEqual([]);
	});

	it("reads the radio's own direction for the horizontal arrows", () => {
		const g = group(3);
		const chosen: number[] = [];
		vi.stubGlobal('getComputedStyle', () => ({ direction: 'rtl' }));
		handleRadioKey(g.press('ArrowLeft', 0), 0, 3, (i) => chosen.push(i));
		expect(chosen).toEqual([1]);
	});

	it('still moves the choice for an event with no target to focus', () => {
		const chosen: number[] = [];
		const e: RadioKeyEvent = { key: 'End', currentTarget: null, preventDefault: () => {} };
		handleRadioKey(e, 0, 3, (i) => chosen.push(i));
		expect(chosen).toEqual([2]);
	});
});
