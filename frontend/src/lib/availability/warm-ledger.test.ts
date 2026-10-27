import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import { createWarmLedger, dueForWarm, WARM_RETRY_MS } from './warm-ledger';

describe('dueForWarm', () => {
	it('is due exactly when never warmed or the retry window has passed', () => {
		fc.assert(
			fc.property(
				fc.option(fc.integer({ min: 0, max: 1e12 }), { nil: undefined }),
				fc.integer({ min: 0, max: 1e12 }),
				fc.integer({ min: 1, max: 1e9 }),
				(sentAt, now, retryMs) => {
					const expected = sentAt === undefined || now - sentAt >= retryMs;
					expect(dueForWarm(sentAt, now, retryMs)).toBe(expected);
				}
			)
		);
	});
});

describe('createWarmLedger', () => {
	it('hands a key in once, and again only when its retry is due', () => {
		const ledger = createWarmLedger(1000);
		expect(ledger.take(['sub:1', 'sub:2'], 0)).toEqual(['sub:1', 'sub:2']);
		expect(ledger.take(['sub:1', 'sub:3'], 999)).toEqual(['sub:3']);
		expect(ledger.take(['sub:1', 'sub:3'], 1000)).toEqual(['sub:1']);
	});

	it('a key forgotten after a failed warm is due at once', () => {
		const ledger = createWarmLedger(1000);
		ledger.take(['sub:1'], 0);
		ledger.forget(['sub:1']);
		expect(ledger.take(['sub:1'], 1)).toEqual(['sub:1']);
	});

	it('keeps sub and dub apart', () => {
		const ledger = createWarmLedger();
		ledger.take(['sub:1'], 0);
		expect(ledger.take(['dub:1'], 1)).toEqual(['dub:1']);
	});

	it('defaults to the session retry window', () => {
		const ledger = createWarmLedger();
		ledger.take(['sub:1'], 0);
		expect(ledger.take(['sub:1'], WARM_RETRY_MS - 1)).toEqual([]);
		expect(ledger.take(['sub:1'], WARM_RETRY_MS)).toEqual(['sub:1']);
	});
});
