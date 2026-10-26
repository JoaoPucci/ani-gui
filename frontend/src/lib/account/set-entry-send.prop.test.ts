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

describe('the re-check after the read, by property', () => {
	/** A held write whose fields are `before` until the tracker is read,
	 *  and `after` from then on — a newer write issued while the read was
	 *  out. */
	function heldAcross(before: WriteField[], after: WriteField[]) {
		let read = false;
		return {
			held: { left: () => (read ? after : before) },
			markRead: () => {
				read = true;
			}
		};
	}

	it('a save sends what the fields after the read leave, and reads only when something was left', async () => {
		await fc.assert(
			fc.asyncProperty(
				save,
				current,
				fields,
				fields,
				fields,
				async (s, row, declared, before, after) => {
					const sent: unknown[] = [];
					const { held, markRead } = heldAcross(before, after);
					let reads = 0;
					const deps = {
						getEntry: async () => {
							reads += 1;
							markRead();
							return row;
						},
						setEntry: async (_p: unknown, _b: unknown, body: unknown) => {
							sent.push(body);
							return null;
						}
					};
					const to = { provider: 'anilist' as const, bearer: 't', kitsuId: '12', held };
					const outcome = await sendSave(deps, to, s, declared).catch((e: unknown) => e);
					expect(reads).toBe(before.length === 0 ? 0 : 1);
					const edit = buildListEdit({
						current: row?.status ?? null,
						statusChanged: s.statusChanged,
						status: s.status,
						progress: s.progress,
						total: s.total
					});
					const expected = before.length === 0 ? null : keepWrite(edit, declared, after);
					if (expected === null) {
						expect(outcome).toBeInstanceOf(SupersededError);
						expect(sent).toEqual([]);
					} else expect(sent).toEqual([{ kitsu_id: '12', ...expected }]);
				}
			)
		);
	});

	it('a removal goes only while every field is its own both before and after the read', async () => {
		await fc.assert(
			fc.asyncProperty(current, fields, fields, async (row, before, after) => {
				const { held, markRead } = heldAcross(before, after);
				let removed = 0;
				let reads = 0;
				const deps = {
					getEntry: async () => {
						reads += 1;
						markRead();
						return row;
					},
					removeEntry: async () => {
						removed += 1;
					}
				};
				const to = { provider: 'anilist' as const, bearer: 't', kitsuId: '12', held };
				const outcome = await sendRemoval(deps, to).catch((e: unknown) => e);
				if (before.length < 2) {
					expect(outcome).toBeInstanceOf(SupersededError);
					expect(reads).toBe(0);
				} else if (row === null) {
					expect(outcome).toBe('neither');
				} else if (after.length < 2) {
					expect(outcome).toBeInstanceOf(SupersededError);
				} else {
					expect(outcome).toBe('ok');
				}
				expect(removed).toBe(outcome === 'ok' ? 1 : 0);
			})
		);
	});
});
