import { describe, expect, it, vi } from 'vitest';
import { joinResumeRefresh, refreshedResume } from './resume-refresh';

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

describe('refreshedResume (a row put right before the page read it)', () => {
	it('moves the position kept under the number it read as before, though it reads as it did', async () => {
		// The home page's probe put the row right before this page read
		// it; nothing on the way moved the recap's position.
		const move = vi.fn();
		const at = { ...entry, ep_no: '12.5' };
		const next = await refreshedResume(
			at,
			'42',
			async () => ({ ep_no: '12.5', former_ep_no: '24.5' }),
			move
		);
		expect(move).toHaveBeenCalledWith('42', 24.5, 12.5);
		expect(next).toBeNull();
	});
});

describe('refreshedResume (no listing behind the lookup)', () => {
	it('warms a fractional row the lookup left as it was, then reads it again', async () => {
		let warmed = false;
		const warm = vi.fn().mockImplementation(async () => {
			warmed = true;
		});
		const next = await refreshedResume(
			entry,
			'42',
			async () => (warmed ? { ep_no: '12.5', former_ep_no: '24.5' } : { ep_no: '24.5' }),
			vi.fn(),
			warm
		);
		expect(warm).toHaveBeenCalledWith(24.5);
		expect(next).toEqual({ ...entry, ep_no: '12.5' });
	});

	it('warms nothing for a whole episode, and nothing more when the warm fails', async () => {
		const warm = vi.fn().mockRejectedValue(new Error('offline'));
		const whole = { ...entry, ep_no: '5' };
		expect(
			await refreshedResume(whole, '42', async () => ({ ep_no: '5' }), vi.fn(), warm)
		).toBeNull();
		expect(warm).not.toHaveBeenCalled();
		expect(
			await refreshedResume(entry, '42', async () => ({ ep_no: '24.5' }), vi.fn(), warm)
		).toBeNull();
		expect(warm).toHaveBeenCalledTimes(1);
	});
});

describe('joinResumeRefresh', () => {
	it('runs once the row and the lookup have both landed, in either order', () => {
		const run = vi.fn();
		const join = joinResumeRefresh(run);
		join.resolved();
		expect(run).not.toHaveBeenCalled();
		join.row(entry);
		expect(run).toHaveBeenCalledWith(entry);
		const other = vi.fn();
		const later = joinResumeRefresh(other);
		later.row(entry);
		expect(other).not.toHaveBeenCalled();
		later.resolved();
		expect(other).toHaveBeenCalledWith(entry);
	});

	it('runs nothing for a visit with no row, and starts over on reset', () => {
		const run = vi.fn();
		const join = joinResumeRefresh(run);
		join.row(null);
		join.resolved();
		expect(run).not.toHaveBeenCalled();
		join.reset();
		join.row(entry);
		expect(run).not.toHaveBeenCalled();
		join.resolved();
		expect(run).toHaveBeenCalledTimes(1);
	});
});

describe('joinResumeRefresh (a second lookup)', () => {
	it('runs again for every lookup that answers, and waits for one going out', () => {
		// The first lookup can go out in the wrong audio before the
		// settings land; the second is the one that puts the row right.
		const run = vi.fn();
		const join = joinResumeRefresh(run);
		join.row(entry);
		join.resolved();
		expect(run).toHaveBeenCalledTimes(1);
		join.unresolved();
		join.row(entry);
		expect(run).toHaveBeenCalledTimes(1);
		join.resolved();
		expect(run).toHaveBeenCalledTimes(2);
	});
});
