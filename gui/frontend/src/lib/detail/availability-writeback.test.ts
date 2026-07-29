import { describe, it, expect } from 'vitest';
import { createAvailabilityWriteback } from './availability-writeback';

const answer = (available: boolean, count: number | null, extras: string[] = []) => ({
	available,
	count,
	extraEpisodes: extras
});

describe('createAvailabilityWriteback', () => {
	it('lets an exact lookup fill the count an approximate re-ask withheld', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:dub');
		const settle = wb.begin();

		// The user re-asked mid-lookup and allmanga answered from the
		// search hit — listed, but the count off it counts halves as
		// whole episodes, so it says nothing certain about the cap.
		const fromRefresh = wb.refresh({ available: true, count: null, extraEpisodes: null });
		expect(fromRefresh).toEqual({ available: true });

		// The lookup then lands with the confirmed count. Nobody has
		// established one, so there is nothing here to protect: the
		// alternative is keeping whatever the row held before.
		expect(settle(answer(true, 12, ['12.5']))).toEqual({
			count: 12,
			extraEpisodes: ['12.5']
		});
	});

	it('does not let a lookup take back a verdict a re-ask settled', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const settle = wb.begin();

		wb.refresh({ available: true, count: null, extraEpisodes: null });

		// The verdict is the re-ask's — it skipped the cache to get it,
		// and this one read through the cache the re-ask was correcting.
		// Only that is this case's claim: the count travelling with a
		// negative answer is superseded along with it, which the
		// superseded-negative case below is what states.
		expect(settle(answer(false, 12))).not.toHaveProperty('available');
	});

	it('does not let a lookup overwrite a count a re-ask confirmed', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const settle = wb.begin();

		wb.refresh({ available: true, count: 9, extraEpisodes: ['9.5'] });

		expect(settle(answer(true, 6, []))).toEqual({});
	});

	it('hands a confirmed re-ask the whole row', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		expect(wb.refresh({ available: true, count: 9, extraEpisodes: ['9.5'] })).toEqual({
			available: true,
			count: 9,
			extraEpisodes: ['9.5']
		});
	});

	it('writes nothing once the question has moved to the other catalogue', () => {
		let context = 'show-1:sub';
		const wb = createAvailabilityWriteback(() => context);
		const settle = wb.begin();

		// sub and dub are catalogued separately, so a count from one
		// says nothing about the other. Only the row this lookup asked
		// about is its to write.
		context = 'show-1:dub';
		expect(settle(answer(true, 12, ['12.5']))).toEqual({});
	});

	it('writes nothing once the page it was asked for is gone', () => {
		let context = 'show-1:sub';
		const wb = createAvailabilityWriteback(() => context);
		const settle = wb.begin();

		context = 'show-2:sub';
		expect(settle(answer(true, 12))).toEqual({});
	});

	it('writes the whole answer when nothing overtook it', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const settle = wb.begin();

		expect(settle(answer(true, 12, ['12.5']))).toEqual({
			available: true,
			count: 12,
			extraEpisodes: ['12.5']
		});
	});

	it('keeps a later lookup free after an earlier one settled', () => {
		// Ordinary lookups do not claim anything against each other —
		// only a re-ask does, because only a re-ask skipped the cache.
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const first = wb.begin();
		const second = wb.begin();

		expect(first(answer(true, 12))).toEqual({
			available: true,
			count: 12,
			extraEpisodes: []
		});
		expect(second(answer(true, 13))).toEqual({
			available: true,
			count: 13,
			extraEpisodes: []
		});
	});

	it('drops the count that came with a superseded negative verdict', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const settle = wb.begin();

		// A re-ask found the show, but only approximately — so it took
		// the verdict and left the cap open.
		wb.refresh({ available: true, count: null, extraEpisodes: null });

		// This lookup says the show is not there at all. Its "no count"
		// is not a measurement of anything; it is the same claim as its
		// verdict, and the verdict has been superseded. Publishing it
		// would put a null cap on a show the re-ask just said exists,
		// and both routes read a null cap as unbounded — every aired
		// tile playable on a show nobody has confirmed a cap for.
		expect(settle(answer(false, null))).toEqual({});
	});

	it('still lets a confirmed count through when only the verdict was superseded', () => {
		const wb = createAvailabilityWriteback(() => 'show-1:sub');
		const settle = wb.begin();

		wb.refresh({ available: true, count: null, extraEpisodes: null });

		// A positive answer's count stands on its own — the lookup
		// counted episodes. Only the verdict belongs to the re-ask.
		expect(settle(answer(true, 12, ['12.5']))).toEqual({
			count: 12,
			extraEpisodes: ['12.5']
		});
	});
});
