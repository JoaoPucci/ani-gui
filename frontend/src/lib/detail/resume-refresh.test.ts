import { describe, expect, it, vi } from 'vitest';
import { refreshedResume } from './resume-refresh';

const entry = { id: 'hianime:show-4', ep_no: '24.5', title: 'Show' };

describe('refreshedResume', () => {
	it('gives the row the episode it reads as now and moves its position', async () => {
		const move = vi.fn();
		const next = await refreshedResume(
			entry,
			'42',
			async () => ({ ep_no: '12.5', former_ep_no: '24.5' }),
			move
		);
		expect(next).toEqual({ ...entry, ep_no: '12.5' });
		expect(move).toHaveBeenCalledWith('42', 24.5, 12.5);
	});

	it('moves nothing when the row names no former number', async () => {
		const move = vi.fn();
		const next = await refreshedResume(entry, '42', async () => ({ ep_no: '12.5' }), move);
		expect(next).toEqual({ ...entry, ep_no: '12.5' });
		expect(move).not.toHaveBeenCalled();
	});

	it('changes nothing when the row reads as it did, is gone, or cannot be read', async () => {
		const move = vi.fn();
		expect(await refreshedResume(entry, '42', async () => ({ ep_no: '24.5' }), move)).toBeNull();
		expect(await refreshedResume(entry, '42', async () => null, move)).toBeNull();
		expect(
			await refreshedResume(entry, '42', vi.fn().mockRejectedValue(new Error('x')), move)
		).toBeNull();
		expect(move).not.toHaveBeenCalled();
	});
});
