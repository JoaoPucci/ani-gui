import { describe, expect, it } from 'vitest';
import { MOCK_RUNS } from './fixtures';
import { currentIn } from '../strip-view';
import { mastheadArt } from './masthead';
import type { MockRun } from './types';

const run = (key: string): MockRun => MOCK_RUNS.find((r) => r.key === key)!;

describe('mastheadArt', () => {
	it('is the entry the franchise is named after, not the one being watched', () => {
		const jojo = run('jojo');
		// The viewer is on Steel Ball Run; the name is the 2012 series'.
		expect(currentIn(jojo, jojo.progress, 'linked')?.entry.id).toBe('49847');
		for (const order of ['linked', 'released'] as const) {
			expect(mastheadArt(jojo, order).id).toBe('7158');
		}
	});

	it("is every run's naming entry, whatever its progress", () => {
		for (const r of MOCK_RUNS) {
			for (const order of ['linked', 'released'] as const) {
				expect(mastheadArt(r, order).id).toBe(r.nameId);
			}
		}
	});
});
