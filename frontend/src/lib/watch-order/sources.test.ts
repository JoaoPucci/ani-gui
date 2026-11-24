import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/api', () => ({
	watchOrderShow: vi.fn(() => Promise.resolve({})),
	watchOrderFranchise: vi.fn(() => Promise.resolve({})),
	watchOrderUpNext: vi.fn(() => Promise.resolve({}))
}));

import { watchOrderFranchise, watchOrderShow, watchOrderUpNext } from '$lib/api';
import { localDay } from './entry';
import { watchOrderSources } from './sources';

describe('watchOrderSources', () => {
	it("reads each page's route for the show, on the viewer's local day", async () => {
		expect(Object.keys(watchOrderSources).sort()).toEqual(['franchise', 'show', 'upNext']);
		const today = localDay(new Date());
		await watchOrderSources.show('7');
		await watchOrderSources.franchise('8');
		await watchOrderSources.upNext('9', 12);
		expect(watchOrderShow).toHaveBeenLastCalledWith('7', { today }, { signal: undefined });
		expect(watchOrderFranchise).toHaveBeenLastCalledWith('8', { today }, { signal: undefined });
		expect(watchOrderUpNext).toHaveBeenLastCalledWith(
			'9',
			{ today, episode: 12 },
			{ signal: undefined }
		);
	});

	it("cancels each read with the visit's signal", async () => {
		const visit = new AbortController();
		await watchOrderSources.show('7', visit.signal);
		await watchOrderSources.franchise('7', visit.signal);
		await watchOrderSources.upNext('7', 3, visit.signal);
		expect(vi.mocked(watchOrderShow).mock.lastCall?.[2]).toEqual({ signal: visit.signal });
		expect(vi.mocked(watchOrderFranchise).mock.lastCall?.[2]).toEqual({ signal: visit.signal });
		expect(vi.mocked(watchOrderUpNext).mock.lastCall?.[2]).toEqual({ signal: visit.signal });
	});
});
