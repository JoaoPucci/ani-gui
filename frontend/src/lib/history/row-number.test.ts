import { describe, expect, it, vi } from 'vitest';
import { refreshedRows } from './row-number';
import type { HistoryEntry } from '$lib/api';

const rows: HistoryEntry[] = [
	{ id: 'a', ep_no: '24.5', title: 'A' },
	{ id: 'b', ep_no: '3', title: 'B' }
];

describe('refreshedRows', () => {
	it('gives the row the episode it reads as now', async () => {
		const next = await refreshedRows(
			'a',
			() => rows,
			async () => ({ ep_no: '12.5' })
		);
		expect(next).toEqual([
			{ id: 'a', ep_no: '12.5', title: 'A' },
			{ id: 'b', ep_no: '3', title: 'B' }
		]);
	});

	it('changes nothing when the row reads as it did, is gone, or cannot be read', async () => {
		expect(
			await refreshedRows(
				'b',
				() => rows,
				async () => ({ ep_no: '3' })
			)
		).toBeNull();
		expect(
			await refreshedRows(
				'a',
				() => rows,
				async () => null
			)
		).toBeNull();
		expect(
			await refreshedRows('a', () => rows, vi.fn().mockRejectedValue(new Error('offline')))
		).toBeNull();
		expect(
			await refreshedRows(
				'z',
				() => rows,
				async () => ({ ep_no: '9' })
			)
		).toBeNull();
	});

	it('reads the rows as they stand when the answer lands', async () => {
		// The strip may have been reloaded while the row was asked.
		let current: HistoryEntry[] | null = rows;
		const next = await refreshedRows(
			'a',
			() => current,
			async () => {
				current = [{ id: 'a', ep_no: '24.5', title: 'A' }];
				return { ep_no: '12.5' };
			}
		);
		expect(next).toEqual([{ id: 'a', ep_no: '12.5', title: 'A' }]);
	});
});
