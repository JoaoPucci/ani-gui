import { describe, expect, test } from 'vitest';
import fc from 'fast-check';
import { failed, landed, type ErrorAction, type ErrorBlock } from './error-block';

const ACTIONS: ErrorAction[] = ['save', 'clear_history', 'clear_images'];

describe('error-block', () => {
	test('a failure puts the block up about that action', () => {
		expect(failed(null, 'clear_history', 'disk full')).toEqual({
			action: 'clear_history',
			detail: 'disk full'
		});
	});

	test('a later failure replaces the block, whatever it was about', () => {
		const block = failed(null, 'clear_history', 'a');
		expect(failed(block, 'save', 'b')).toEqual({ action: 'save', detail: 'b' });
	});

	test('an action that lands takes down its own failure', () => {
		for (const action of ACTIONS) {
			expect(landed(failed(null, action, 'x'), action)).toBeNull();
		}
	});

	test('an action that lands leaves another action’s failure up', () => {
		const clear = failed(null, 'clear_history', 'x');
		expect(landed(clear, 'save')).toBe(clear);
		expect(landed(clear, 'clear_images')).toBe(clear);
		const save = failed(null, 'save', 'y');
		expect(landed(save, 'clear_history')).toBe(save);
	});

	test('a landing with the block down leaves it down', () => {
		for (const action of ACTIONS) expect(landed(null, action)).toBeNull();
	});

	test('property: the block shows the last failure until its own action lands', () => {
		const event = fc.record({
			action: fc.constantFrom(...ACTIONS),
			ok: fc.boolean(),
			detail: fc.string()
		});
		fc.assert(
			fc.property(fc.array(event, { maxLength: 20 }), (events) => {
				let block: ErrorBlock = null;
				let lastFailure: { action: ErrorAction; detail: string } | null = null;
				for (const e of events) {
					if (e.ok) {
						block = landed(block, e.action);
						if (lastFailure?.action === e.action) lastFailure = null;
					} else {
						block = failed(block, e.action, e.detail);
						lastFailure = { action: e.action, detail: e.detail };
					}
					expect(block).toEqual(lastFailure);
				}
			})
		);
	});
});
