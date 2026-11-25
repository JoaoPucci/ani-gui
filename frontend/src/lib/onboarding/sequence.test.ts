import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { MAX_STEPS, actionsFor, startSequence, stepSequence, type SeqEvent } from './sequence';

describe('sequence', () => {
	it('walks Next to Done, and Back stops at the first step', () => {
		let s = startSequence(3);
		s = stepSequence(s, 'back');
		expect(s.index).toBe(0);
		s = stepSequence(stepSequence(s, 'next'), 'next');
		expect(s).toEqual({ index: 2, total: 3, done: false });
		expect(actionsFor(s)).toEqual({ skip: false, back: true, last: true });
		expect(stepSequence(s, 'next').done).toBe(true);
	});
	it('Skip or a close ends it from any step', () => {
		expect(stepSequence(startSequence(3), 'skip').done).toBe(true);
		expect(stepSequence(startSequence(2), 'close').done).toBe(true);
	});
	it('refuses more than three steps', () => {
		expect(() => startSequence(MAX_STEPS + 1)).toThrow(RangeError);
		expect(() => startSequence(0)).toThrow(RangeError);
	});
	it('stays in range and never comes back from done (property)', () => {
		fc.assert(
			fc.property(
				fc.integer({ min: 1, max: MAX_STEPS }),
				fc.array(fc.constantFrom<SeqEvent>('next', 'back', 'skip', 'close')),
				(total, events) => {
					let s = startSequence(total);
					let wasDone = false;
					for (const e of events) {
						s = stepSequence(s, e);
						if (wasDone && !s.done) return false;
						wasDone = s.done;
						if (s.index < 0 || s.index >= total) return false;
					}
					return true;
				}
			)
		);
	});
});
