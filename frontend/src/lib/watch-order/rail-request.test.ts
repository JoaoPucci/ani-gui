import { describe, expect, it, vi } from 'vitest';
import type { Handoff } from '$lib/api';
import { askRow, railRequest } from './rail-request';

const match = (id: string) => ({ id, status: 'finished', episode_count: 13 });
const never = () => false;

describe('askRow', () => {
	it("asks the player's storage about the row's own last episode, by its match", () => {
		const leftPartWay = vi.fn(() => true);
		expect(askRow({ id: 'r1', ep_no: '13' }, match('43078'), 14, leftPartWay)).toEqual({
			row_id: 'r1',
			matched: { id: '43078', status: 'finished', episode_count: 13 },
			ep_no: '13',
			playable_count: 14,
			left_part_way: true
		});
		expect(leftPartWay).toHaveBeenCalledWith('43078', 13);
	});

	it('reads an extra and a range by the episode they lead with', () => {
		const leftPartWay = vi.fn(() => false);
		askRow({ id: 'r1', ep_no: '12.5' }, match('7'), undefined, leftPartWay);
		askRow({ id: 'r2', ep_no: '3-4' }, match('7'), undefined, leftPartWay);
		expect(leftPartWay.mock.calls).toEqual([
			['7', 12.5],
			['7', 3]
		]);
	});

	it('asks nothing of the storage for a row not matched, or with no episode', () => {
		const leftPartWay = vi.fn(() => true);
		for (const m of [null, undefined]) {
			expect(askRow({ id: 'r1', ep_no: '13' }, m, null, leftPartWay)).toMatchObject({
				matched: null,
				playable_count: null,
				left_part_way: false
			});
		}
		expect(askRow({ id: 'r1', ep_no: 'Special' }, match('7'), null, leftPartWay)).toMatchObject({
			left_part_way: false
		});
		expect(leftPartWay).not.toHaveBeenCalled();
	});
});

describe('railRequest', () => {
	const handoff = { finished: { id: '1' }, next: { id: '2' } } as unknown as Handoff;

	it('asks nothing for a rail with no rows', () => {
		expect(railRequest([], [], {}, {}, {}, never)).toBeNull();
	});

	it('sends the rows in rail order, every history row, their match ids and the hand-offs read', () => {
		const rows = [
			{ id: 'b', ep_no: '13' },
			{ id: 'a', ep_no: '2' }
		];
		const history = [{ id: 'a', kitsu_id: '10' }, { id: 'b' }, { id: 'c', kitsu_id: '30' }];
		const body = railRequest(
			rows,
			history,
			{ a: match('10'), b: null },
			{ b: 14 },
			{ '10': handoff, '20': null },
			never
		);
		expect(body).toEqual({
			rows: [
				{ row_id: 'b', matched: null, ep_no: '13', playable_count: 14, left_part_way: false },
				{
					row_id: 'a',
					matched: { id: '10', status: 'finished', episode_count: 13 },
					ep_no: '2',
					playable_count: null,
					left_part_way: false
				}
			],
			history: [
				{ id: 'a', kitsu_id: '10' },
				{ id: 'b', kitsu_id: null },
				{ id: 'c', kitsu_id: '30' }
			],
			// `c` is still being matched: absent, so the played set waits.
			matches: { a: '10', b: null },
			series: { '10': handoff, '20': null }
		});
	});
});
