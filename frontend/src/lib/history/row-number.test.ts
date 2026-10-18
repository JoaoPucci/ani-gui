import { describe, expect, it, vi } from 'vitest';
import { refreshedRows } from './row-number';
import { latestReads } from './latest-call';
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

describe('refreshedRows (a position kept under the old number)', () => {
	it('moves the position before the rows it gives are read', async () => {
		const events: string[] = [];
		const next = await refreshedRows(
			'a',
			() => rows,
			async () => ({ ep_no: '12.5', former_ep_no: '24.5' }),
			(from, to) => events.push(`move ${from}->${to}`)
		);
		expect(events).toEqual(['move 24.5->12.5']);
		expect(next?.[0].ep_no).toBe('12.5');
	});

	it('moves it, and gives the rows anew, for a row that reads as it did', async () => {
		// Put right before the page read it: the row is "12.5" already,
		// and the card has to be offered again once the position moved.
		const move = vi.fn().mockReturnValue(true);
		const at: HistoryEntry[] = [{ id: 'a', ep_no: '12.5', title: 'A' }];
		const next = await refreshedRows(
			'a',
			() => at,
			async () => ({ ep_no: '12.5', former_ep_no: '24.5' }),
			move
		);
		expect(move).toHaveBeenCalledWith(24.5, 12.5);
		expect(next).toEqual(at);
		expect(next).not.toBe(at);
	});

	it('gives nothing back when nothing moved, though the row names a former number', async () => {
		// The stamp keeps naming its former number; once the position has
		// moved, a read that moves nothing changes nothing.
		const at: HistoryEntry[] = [{ id: 'a', ep_no: '12.5', title: 'A' }];
		const next = await refreshedRows(
			'a',
			() => at,
			async () => ({ ep_no: '12.5', former_ep_no: '24.5' }),
			() => false
		);
		expect(next).toBeNull();
	});
});

describe('refreshedRows (two reads of one card out at once)', () => {
	it('leaves the card on the newer answer when the older one lands last', async () => {
		// The match's read goes out before the probe repairs the stamp
		// and is answered after the probe's own read: the card keeps the
		// probe's "12.5", not the "24.5" from before the repair, and no
		// position moves for the older answer.
		const answers: ((v: { ep_no: string; former_ep_no?: string }) => void)[] = [];
		const read = latestReads(
			() => new Promise<{ ep_no: string; former_ep_no?: string } | null>((r) => answers.push(r))
		);
		const move = vi.fn(() => true);
		let card: HistoryEntry[] = rows;
		const older = refreshedRows('a', () => card, read, move);
		const newer = refreshedRows('a', () => card, read, move);
		answers[1]({ ep_no: '12.5', former_ep_no: '24.5' });
		card = (await newer) ?? card;
		answers[0]({ ep_no: '24.5' });
		card = (await older) ?? card;
		expect(card.find((r) => r.id === 'a')?.ep_no).toBe('12.5');
		expect(move).toHaveBeenCalledTimes(1);
	});
});
