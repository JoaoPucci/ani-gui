// The rail fake the hand-off scenarios answer Home's rail with
// (src/test/watch-order-rail-fake.ts) answers as the backend does:
// every rail request a scenario sent was recorded with the backend's
// answer (tests/fixtures/watch-order/scenarios/*.json `rails`, checked
// by backend/src/watch_order/scenarios_test.rs), and the fake gives
// the same answer to each.

import { describe, it, expect } from 'vitest';

import { railView } from '../../src/test/watch-order-rail-fake';
import { canonical, readSuite, suiteNames } from './watch-order-answers';

const rails = suiteNames().flatMap((suite) => readSuite(suite).rails.map((r) => ({ suite, ...r })));

describe("the rail fake, against the backend's recorded answers", () => {
	it('has recorded rails to be held to', () => {
		expect(rails.length).toBeGreaterThan(0);
	});

	it('answers every recorded rail request as the backend did', () => {
		for (const r of rails) {
			expect(canonical(railView(r.request)), `${r.suite}: ${r.name}`).toBe(canonical(r.answer));
		}
	});
});
