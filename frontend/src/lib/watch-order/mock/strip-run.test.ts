import { describe, expect, it } from 'vitest';
import { stripRunFor } from './strip-run';

describe('stripRunFor', () => {
	it("finds a member's run, and a side story's through its parents", () => {
		expect(stripRunFor('46598')?.key).toBe('jojo');
		expect(stripRunFor('6928')?.key).toBe('fate');
		expect(stripRunFor('11759')?.key).toBe('gintama');
		expect(stripRunFor('12064')?.key).toBe('jojo');
	});

	it('finds nothing for an entry with no strip', () => {
		// A Bleach film: around the run, but no parent story recorded.
		expect(stripRunFor('1515')).toBeNull();
		expect(stripRunFor('999999')).toBeNull();
	});
});
