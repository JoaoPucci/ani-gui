import { describe, expect, it } from 'vitest';
import { StepWatch } from './step-watch';

describe('StepWatch', () => {
	it('says nothing on the first step, and announces each later one', () => {
		const w = new StepWatch();
		expect(w.seen(1, 'one')).toEqual({ refocus: false, announce: null });
		expect(w.seen(2, 'two')).toEqual({ refocus: true, announce: 'two' });
		// The same step read again (its words resolved anew) moves nothing.
		expect(w.seen(2, 'two')).toEqual({ refocus: false, announce: 'two' });
	});

	it('announces the first step again once the sequence has moved, and refocuses on each change', () => {
		const w = new StepWatch();
		w.seen(1, 'one');
		w.seen(2, 'two');
		expect(w.seen(1, 'one')).toEqual({ refocus: true, announce: 'one' });
	});
});
