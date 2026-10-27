import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import {
	changeOf,
	entryNeedsReread,
	railNeedsReread,
	writeEnd,
	type WriteEnd
} from './list-changes';
import { SupersededError } from './save-when-allowed';
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
	it('is wanted exactly when a tracker the row shows took the write', () => {
		fc.assert(
			fc.property(
				fc.subarray(PROVIDERS, { minLength: 1 }),
				fc.subarray(PROVIDERS),
				fc.subarray(PROVIDERS),
				(sent, tookIt, shown) => {
					const landed = sent.filter((p) => tookIt.includes(p));
					const wanted = railNeedsReread(
						{ kitsuId: '7', providers: sent, landed, source: 'editor' },
						shown
					);
					expect(wanted).toBe(landed.some((p) => shown.includes(p)));
				}
			)
		);
	});
});

describe('writeEnd, by property', () => {
	it('is landed for any answer, dropped for a superseded write, and failed for anything else thrown', () => {
		fc.assert(
			fc.property(fc.anything(), (value) => {
				expect(writeEnd({ status: 'fulfilled', value })).toBe('landed');
				expect(writeEnd({ status: 'rejected', reason: value })).toBe('failed');
			})
		);
		expect(writeEnd({ status: 'rejected', reason: new SupersededError() })).toBe('dropped');
	});
});

describe('entryNeedsReread, by property', () => {
	it("is wanted exactly for a progress write of the page's own show", () => {
		fc.assert(
			fc.property(
				fc.constantFrom('1', '2', '3'),
				fc.option(fc.constantFrom('1', '2', '3', ''), { nil: null }),
				fc.constantFrom('editor', 'progress' as const),
				(changed, onPage, source) => {
					const wanted = entryNeedsReread(
						{ kitsuId: changed, providers: ['anilist'], landed: ['anilist'], source },
						onPage
					);
					expect(wanted).toBe(source === 'progress' && onPage === changed);
				}
			)
		);
	});
});
