import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/api', () => ({
	historyByKitsuEach: vi.fn(),
	kitsuFranchise: vi.fn()
}));

import { historyByKitsuEach, kitsuFranchise } from '$lib/api';
import { watchOrderSources } from './sources';

describe('watchOrderSources', () => {
	it('reads the franchise route, and the rows its entries resume from', () => {
		expect(watchOrderSources.franchise).toBe(kitsuFranchise);
		expect(watchOrderSources.named).toBe(historyByKitsuEach);
		expect(watchOrderSources.history).toBeUndefined();
	});
});
