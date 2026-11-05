// What the player's slip shows, and the rows hidden until restart.

import { beforeEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	__resetHiddenRowsForTests,
	hideRow,
	isRowHidden,
	slipState,
	type SlipInputs
} from './guess-slip';

beforeEach(() => __resetHiddenRowsForTests());

const inputs = fc.record({
	row: fc.option(fc.constantFrom('hianime:a-1', 'b-2'), { nil: null }),
	aside: fc.boolean(),
	linked: fc.boolean(),
	settled: fc.boolean(),
	offered: fc.boolean()
});

describe('slipState', () => {
	it('shows nothing for a session no guess opened, or once hidden', () => {
		fc.assert(
			fc.property(inputs, (s: SlipInputs) => {
				if (s.row === null || s.aside) expect(slipState(s)).toBe('none');
			})
		);
	});

	it('notes a link elsewhere before anything else it could show', () => {
		fc.assert(
			fc.property(inputs, (s: SlipInputs) => {
				if (s.row !== null && !s.aside && s.linked) expect(slipState(s)).toBe('linked');
			})
		);
	});

	it('holds its room until the offer is decided, then shows the guess while offered', () => {
		const base = { row: 'hianime:a-1', aside: false, linked: false };
		expect(slipState({ ...base, settled: false, offered: false })).toBe('holding');
		expect(slipState({ ...base, settled: false, offered: true })).toBe('holding');
		expect(slipState({ ...base, settled: true, offered: true })).toBe('guess');
		expect(slipState({ ...base, settled: true, offered: false })).toBe('none');
	});
});

describe('hidden rows', () => {
	it('are hidden until the app restarts, each row on its own', () => {
		expect(isRowHidden('hianime:a-1')).toBe(false);
		hideRow('hianime:a-1');
		expect(isRowHidden('hianime:a-1')).toBe(true);
		expect(isRowHidden('b-2')).toBe(false);
		expect(isRowHidden(null)).toBe(false);
		__resetHiddenRowsForTests();
		expect(isRowHidden('hianime:a-1')).toBe(false);
	});
});
