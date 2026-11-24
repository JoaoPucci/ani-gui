// The hero pick fake the acceptance tier answers Home's pick with
// (src/test/hero-pick-fake.ts) answers as the backend does: on each of
// the backend's golden cases (backend/tests/fixtures/hero-pick/cases.json,
// checked against the backend's pick by backend/src/hero/golden_test.rs)
// it makes the same pick, walk included.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve as resolvePath } from 'node:path';

import { heroPick } from '../../src/test/hero-pick-fake';
import type { HeroPick, HeroShow } from '../../src/lib/api';

interface Case {
	name: string;
	airing_first: boolean;
	shows: HeroShow[];
	expected: HeroPick;
}

const CASES: Case[] = JSON.parse(
	readFileSync(
		resolvePath(__dirname, '../../../backend/tests/fixtures/hero-pick/cases.json'),
		'utf-8'
	)
);

describe("the hero pick fake, against the backend's golden cases", () => {
	it('has golden cases to be held to, under both values of the preference', () => {
		expect(CASES.some((c) => c.airing_first)).toBe(true);
		expect(CASES.some((c) => !c.airing_first)).toBe(true);
	});

	it.each(CASES.map((c) => [`${c.name} (airing first ${c.airing_first})`, c] as const))(
		'%s',
		(_, c) => {
			expect(heroPick(c.shows, c.airing_first)).toEqual(c.expected);
		}
	);
});
