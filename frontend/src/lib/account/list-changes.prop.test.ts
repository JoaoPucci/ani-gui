import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { changeOf, railNeedsReread, type WriteEnd } from './list-changes';
import type { Provider } from './types';

const PROVIDERS: Provider[] = ['anilist', 'mal', 'inhouse'];
const end = fc.constantFrom<WriteEnd>('landed', 'failed', 'dropped');
const ends = fc.record(
	{ anilist: end, mal: end, inhouse: end },
	{ requiredKeys: [] }
) as fc.Arbitrary<Partial<Record<Provider, WriteEnd>>>;

describe('changeOf, by property', () => {
	it('names exactly the trackers whose write was sent, once each, in tracker order', () => {
		fc.assert(
			fc.property(ends, fc.constantFrom('editor', 'progress' as const), (byTracker, source) => {
				const sent = PROVIDERS.filter(
					(p) => byTracker[p] === 'landed' || byTracker[p] === 'failed'
				);
				const change = changeOf('7', source, byTracker);
				if (sent.length === 0) {
					expect(change).toBeNull();
				} else {
					expect(change).toEqual({
						kitsuId: '7',
						providers: sent,
						landed: PROVIDERS.filter((p) => byTracker[p] === 'landed'),
						source
					});
				}
			})
		);
	});
});

describe('railNeedsReread, by property', () => {
	it('is wanted exactly when the change and the row share a tracker', () => {
		fc.assert(
			fc.property(
				fc.subarray(PROVIDERS, { minLength: 1 }),
				fc.subarray(PROVIDERS),
				(changed, shown) => {
					const wanted = railNeedsReread(
						{ kitsuId: '7', providers: changed, landed: changed, source: 'editor' },
						shown
					);
					expect(wanted).toBe(changed.some((p) => shown.includes(p)));
				}
			)
		);
	});
});
