import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { stepWords } from './words';

/** The backend's registry, read as the arch check reads it. */
interface Decl {
	id: string;
	places: string[];
}
const REGISTRY: Decl[] = JSON.parse(
	readFileSync(resolve(__dirname, '../../../../backend/src/onboarding/tips.json'), 'utf8')
);

describe('stepWords', () => {
	it('has a title and a body for every step of every declared tip', () => {
		for (const tip of REGISTRY) {
			tip.places.forEach((_, i) => {
				const words = stepWords(tip.id, i + 1);
				expect(words, `${tip.id} step ${i + 1}`).not.toBeNull();
				expect(words!.title().length).toBeGreaterThan(0);
				expect(words!.body().length).toBeGreaterThan(0);
			});
		}
	});

	it('has none past a tip’s last step, or for an id no tip has', () => {
		for (const tip of REGISTRY) expect(stepWords(tip.id, tip.places.length + 1)).toBeNull();
		expect(stepWords('no-such-tip', 1)).toBeNull();
	});
});
