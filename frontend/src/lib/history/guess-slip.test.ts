// What the player's slip shows, and the rows hidden until restart.

import { beforeEach, describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	__resetHiddenRowsForTests,
	hideRow,
	isRowHidden,
	notePick,
	noteSwitch,
	pickedRow,
	slipState,
	takeSwitch,
	type SlipInputs
} from './guess-slip';

beforeEach(() => __resetHiddenRowsForTests());

const inputs = fc.record({
	row: fc.option(fc.constantFrom('hianime:a-1', 'b-2'), { nil: null }),
	aside: fc.boolean(),
	linked: fc.boolean(),
	switched: fc.boolean(),
	settled: fc.boolean(),
	offered: fc.boolean(),
	asking: fc.boolean()
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

	it('holds its room until the offer is decided, then asks while the offer still asks', () => {
		const base = { row: 'hianime:a-1', aside: false, linked: false, switched: false };
		expect(slipState({ ...base, settled: false, offered: false, asking: false })).toBe('holding');
		expect(slipState({ ...base, settled: false, offered: true, asking: true })).toBe('holding');
		expect(slipState({ ...base, settled: true, offered: true, asking: true })).toBe('guess');
		expect(slipState({ ...base, settled: true, offered: false, asking: false })).toBe('none');
		// An offer kept only to change the show again asks nothing.
		expect(slipState({ ...base, settled: true, offered: true, asking: false })).toBe('none');
	});

	it('holds no room for a session no guess opened, which can only change a named show again', () => {
		const base = { row: 'hianime:a-1', aside: false, linked: false, switched: false };
		expect(slipState({ ...base, settled: false, offered: false, asking: false, asks: false })).toBe(
			'none'
		);
		expect(slipState({ ...base, settled: true, offered: true, asking: false, asks: false })).toBe(
			'none'
		);
		// A pick kept watching past still says so.
		expect(
			slipState({ ...base, linked: true, settled: true, offered: true, asking: false, asks: false })
		).toBe('linked');
	});

	it('says once that the show was switched, before the question or the held room', () => {
		fc.assert(
			fc.property(inputs, (s: SlipInputs) => {
				if (s.row !== null && !s.aside && !s.linked && s.switched)
					expect(slipState(s)).toBe('switched');
			})
		);
	});

	it('asks the question only for an offer that asks', () => {
		fc.assert(
			fc.property(inputs, (s: SlipInputs) => {
				if (slipState(s) === 'guess') expect(s.offered && s.asking && s.settled).toBe(true);
			})
		);
	});
});

describe('rows picked on the player', () => {
	it('are remembered until the app restarts', () => {
		expect(pickedRow('hianime:a-1')).toBe(false);
		notePick('hianime:a-1');
		expect(pickedRow('hianime:a-1')).toBe(true);
		expect(pickedRow('b-2')).toBe(false);
		expect(pickedRow(null)).toBe(false);
		__resetHiddenRowsForTests();
		expect(pickedRow('hianime:a-1')).toBe(false);
	});
});

describe('a switch to the picked show', () => {
	it('is told once, to the session of its row', () => {
		noteSwitch('hianime:a-1', 'JoJo 2000');
		expect(takeSwitch('b-2')).toBeNull();
		expect(takeSwitch('hianime:a-1')).toBeNull();
		noteSwitch('hianime:a-1', 'JoJo 2000');
		expect(takeSwitch('hianime:a-1')).toBe('JoJo 2000');
		expect(takeSwitch('hianime:a-1')).toBeNull();
		expect(takeSwitch(null)).toBeNull();
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
