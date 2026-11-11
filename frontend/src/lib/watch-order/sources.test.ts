import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/api', () => ({
	historyByKitsuEach: vi.fn(() => Promise.resolve({})),
	kitsuFranchise: vi.fn(() => Promise.resolve({}))
}));

import { historyByKitsuEach, kitsuFranchise } from '$lib/api';
import { watchOrderSources } from './sources';

describe('watchOrderSources', () => {
	it('reads the franchise route, and the rows its entries resume from', async () => {
		expect(Object.keys(watchOrderSources).sort()).toEqual(['franchise', 'named']);
		await watchOrderSources.franchise('7');
		await watchOrderSources.named(['7', '8']);
		expect(vi.mocked(kitsuFranchise).mock.calls[0]?.[0]).toBe('7');
		expect(vi.mocked(historyByKitsuEach).mock.calls[0]?.[0]).toEqual(['7', '8']);
	});

	it("cancels both reads with the visit's signal", async () => {
		const visit = new AbortController();
		await watchOrderSources.franchise('7', visit.signal);
		await watchOrderSources.named(['7', '8'], visit.signal);
		expect(kitsuFranchise).toHaveBeenLastCalledWith('7', { signal: visit.signal });
		expect(historyByKitsuEach).toHaveBeenLastCalledWith(['7', '8'], { signal: visit.signal });
	});
});
