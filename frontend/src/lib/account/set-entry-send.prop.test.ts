import { describe, expect, it } from 'vitest';
import fc from 'fast-check';

import { sendRemoval, sendSave } from './set-entry-send';
import { buildListEdit } from './list-entry-edit';
import { keepWrite, type WriteField } from './write-fields';
import { SupersededError } from './save-when-allowed';
import type { ListStatus } from './types';

const status = fc.constantFrom<ListStatus>(
	'planning',
	'watching',
	'completed',
	'paused',
	'dropped'
);
const fields = fc.subarray<WriteField>(['status', 'progress']);
const save = fc.record({
	status,
	statusChanged: fc.boolean(),
	progress: fc.nat(24),
	total: fc.option(fc.integer({ min: 1, max: 24 }), { nil: null })
});
const current = fc.option(fc.record({ status, progress: fc.nat(24) }), { nil: null });

describe('sendSave, by property', () => {
	it('sends what is left of the edit the read decides, or nothing when nothing is', async () => {
		await fc.assert(
			fc.asyncProperty(save, current, fields, fields, async (s, row, declared, left) => {
				const sent: unknown[] = [];
				let reads = 0;
				const deps = {
					getEntry: async () => {
						reads += 1;
						return row;
					},
					setEntry: async (_p: unknown, _b: unknown, body: unknown) => {
						sent.push(body);
						return null;
					}
				};
				const to = {
					provider: 'anilist' as const,
					bearer: 't',
					kitsuId: '12',
					held: { left: () => left }
				};
				const outcome = await sendSave(deps, to, s, declared).catch((e: unknown) => e);
				const edit = buildListEdit({
					current: row?.status ?? null,
					statusChanged: s.statusChanged,
					status: s.status,
					progress: s.progress,
					total: s.total
				});
				const expected = keepWrite(edit, declared, left);
				// Nothing left, and the tracker is not even read.
				expect(reads).toBe(left.length === 0 ? 0 : 1);
				if (expected === null || left.length === 0) {
					expect(outcome).toBeInstanceOf(SupersededError);
					expect(sent).toEqual([]);
				} else {
					expect(sent).toEqual([{ kitsu_id: '12', ...expected }]);
				}
			})
		);
	});
});

describe('sendRemoval, by property', () => {
	it('removes a row that is there only while every field is still its own, reading only then', async () => {
		await fc.assert(
			fc.asyncProperty(current, fields, async (row, left) => {
				let removed = 0;
				let reads = 0;
				const deps = {
					getEntry: async () => {
						reads += 1;
						return row;
					},
					removeEntry: async () => {
						removed += 1;
					}
				};
				const to = {
					provider: 'anilist' as const,
					bearer: 't',
					kitsuId: '12',
					held: { left: () => left }
				};
				const outcome = await sendRemoval(deps, to).catch((e: unknown) => e);
				if (left.length < 2) {
					expect(outcome).toBeInstanceOf(SupersededError);
					expect(reads).toBe(0);
				} else {
					expect(outcome).toBe(row === null ? 'neither' : 'ok');
					expect(reads).toBe(1);
				}
				expect(removed).toBe(outcome === 'ok' ? 1 : 0);
			})
		);
	});
});
