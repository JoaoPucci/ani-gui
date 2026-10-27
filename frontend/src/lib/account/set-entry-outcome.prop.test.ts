import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { failureResult, outcomeFlags, tallyFanout, type FanoutResult } from './set-entry-outcome';

const result = fc.constantFrom<FanoutResult>(
	'ok',
	'failed',
	'ratelimited',
	'superseded',
	'neither'
);
const results = fc.array(result, { maxLength: 6 });

describe('tallyFanout and outcomeFlags, by property', () => {
	it('counts landed and unconfirmed writes, and leaves superseded and inapplicable ones out', () => {
		fc.assert(
			fc.property(results, (rs) => {
				const t = tallyFanout(rs);
				expect(t.ok).toBe(rs.filter((r) => r === 'ok').length);
				expect(t.failed).toBe(rs.filter((r) => r === 'failed' || r === 'ratelimited').length);
				expect(t.ok + t.failed).toBeLessThanOrEqual(rs.length);
				expect(t.rateLimited).toBe(rs.includes('ratelimited'));
			})
		);
	});

	it('flags only what is there, and only as true', () => {
		fc.assert(
			fc.property(results, (rs) => {
				const flags = outcomeFlags(rs);
				expect(flags.rateLimited === true).toBe(rs.includes('ratelimited'));
				expect(flags.superseded === true).toBe(rs.includes('superseded'));
				expect(Object.values(flags).every((v) => v === true)).toBe(true);
			})
		);
	});
});

describe('failureResult, by property', () => {
	it('is superseded when the write was, else rate-limited for a 429, else failed', () => {
		fc.assert(
			fc.property(fc.integer({ min: 100, max: 599 }), fc.boolean(), (status, superseded) => {
				const error = Object.assign(new Error('x'), { status });
				const r = failureResult(error, superseded);
				if (superseded) expect(r).toBe('superseded');
				else expect(r).toBe(status === 429 ? 'ratelimited' : 'failed');
			})
		);
		fc.assert(
			fc.property(fc.anything(), (value) => {
				fc.pre(!(value instanceof Error));
				expect(failureResult(value, false)).toBe('failed');
			})
		);
	});
});
