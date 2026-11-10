import { describe, expect, it } from 'vitest';
import fc from 'fast-check';
import {
	discoveryView,
	loadDiscoveryRows,
	LOADING,
	settleRow,
	type DiscoveryLoad,
	type RowLoad,
	type RowSlot
} from './discovery';

const ready = (...items: string[]): RowLoad<string> => ({ kind: 'ready', items });
const FAILED: RowLoad<string> = { kind: 'failed' };
const SKELETON: RowSlot<string> = { kind: 'skeleton' };
const ABSENT: RowSlot<string> = { kind: 'absent' };
const shown = (...items: string[]): RowSlot<string> => ({ kind: 'shown', items });

describe('discoveryView', () => {
	it('holds a skeleton in every row while both are loading', () => {
		expect(discoveryView([LOADING, LOADING])).toEqual({
			slots: [SKELETON, SKELETON],
			nothing: false
		});
	});

	it('keeps a row that has landed behind a skeleton while a row above it is still loading', () => {
		// Top rated is usually cached longer than Trending, so it lands
		// first; drawn on its own it would be pushed down when Trending
		// arrives above it.
		expect(discoveryView([LOADING, ready('r1')]).slots).toEqual([SKELETON, SKELETON]);
	});

	it('shows both rows together once the row above lands', () => {
		expect(discoveryView([ready('t1'), ready('r1')]).slots).toEqual([shown('t1'), shown('r1')]);
	});

	it('shows the top row as soon as it lands, with the row below still a skeleton', () => {
		// Nothing above it can move, and the skeleton below fills in
		// place.
		expect(discoveryView([ready('t1'), LOADING]).slots).toEqual([shown('t1'), SKELETON]);
	});

	it('drops a failed row and lets the row below show', () => {
		expect(discoveryView([FAILED, ready('r1')])).toEqual({
			slots: [ABSENT, shown('r1')],
			nothing: false
		});
	});

	it('drops a row the availability filter emptied', () => {
		expect(discoveryView([ready(), ready('r1')]).slots).toEqual([ABSENT, shown('r1')]);
	});

	it('drops a failed row below a loading one rather than holding a skeleton for it', () => {
		expect(discoveryView([LOADING, FAILED]).slots).toEqual([SKELETON, ABSENT]);
	});

	it('says there is nothing to browse when every row failed', () => {
		expect(discoveryView([FAILED, FAILED])).toEqual({ slots: [ABSENT, ABSENT], nothing: true });
	});

	it('says there is nothing to browse when every row came back empty', () => {
		expect(discoveryView([ready(), ready()]).nothing).toBe(true);
	});

	it('says so when one row came back empty and the other failed', () => {
		expect(discoveryView([ready(), FAILED]).nothing).toBe(true);
	});

	it('does not say so while a row is still loading', () => {
		expect(discoveryView([FAILED, LOADING]).nothing).toBe(false);
	});
});

describe('discoveryView, as rows settle in any order', () => {
	const settled = fc.oneof(
		fc.constant<RowLoad<string>>(FAILED),
		fc.array(fc.string({ maxLength: 3 }), { maxLength: 3 }).map((items) => ready(...items))
	);
	// The rows' final answers, and the order they arrive in.
	const run = fc.array(settled, { minLength: 1, maxLength: 5 }).chain((finals) =>
		fc
			.shuffledSubarray(
				finals.map((_, i) => i),
				{ minLength: finals.length, maxLength: finals.length }
			)
			.map((order) => ({ finals, order }))
	);

	/** Every view the page draws on the way, from all-loading to all-settled. */
	function views(finals: RowLoad<string>[], order: number[]) {
		const loads: RowLoad<string>[] = finals.map(() => LOADING);
		const seen = [discoveryView(loads)];
		for (const i of order) {
			loads[i] = finals[i];
			seen.push(discoveryView([...loads]));
		}
		return seen;
	}

	it('never moves a row once it is shown', () => {
		fc.assert(
			fc.property(run, ({ finals, order }) => {
				const seen = views(finals, order);
				seen.forEach((view, k) => {
					view.slots.forEach((slot, i) => {
						if (slot.kind !== 'shown') return;
						// Everything drawn from the top down to this row stays
						// exactly as it is in every later view.
						for (const later of seen.slice(k + 1)) {
							expect(later.slots.slice(0, i + 1)).toEqual(view.slots.slice(0, i + 1));
						}
					});
				});
			})
		);
	});

	it('ends with every row that has something to show, shown in its place', () => {
		fc.assert(
			fc.property(run, ({ finals, order }) => {
				const last = views(finals, order).at(-1)!;
				const expected = finals.map((f) =>
					f.kind === 'ready' && f.items.length > 0 ? shown(...f.items) : ABSENT
				);
				expect(last.slots).toEqual(expected);
				expect(last.nothing).toBe(expected.every((s) => s.kind === 'absent'));
			})
		);
	});
});

describe('settleRow', () => {
	it('settles an answer as a ready row', async () => {
		await expect(settleRow(Promise.resolve(['a']))).resolves.toEqual(ready('a'));
	});

	it('settles a failure as a failed row instead of rejecting', async () => {
		await expect(settleRow(Promise.reject(new Error('offline')))).resolves.toEqual(FAILED);
	});
});

describe('loadDiscoveryRows', () => {
	type Config = { mode: 'sub' | 'dub' };
	const pickMode = (c: Config | null) => c?.mode ?? 'sub';

	/** A load whose events are recorded in order. */
	function recorder(over: Partial<DiscoveryLoad<string, Config>> & { held?: Config | null } = {}) {
		const events: string[] = [];
		let held: Config | null = over.held ?? null;
		const load: DiscoveryLoad<string, Config> = {
			config: () => held,
			readConfig: async () => {
				events.push('read settings');
				return { mode: 'dub' };
			},
			keepConfig: (c) => {
				events.push(`keep ${c.mode}`);
				held = c;
			},
			pickMode,
			rows: [
				async (mode) => {
					events.push(`ask row 0 in ${mode}`);
					return ['t1'];
				},
				async (mode) => {
					events.push(`ask row 1 in ${mode}`);
					return ['r1'];
				}
			],
			onRow: (i, row) => events.push(`row ${i} ${row.kind}`),
			...over
		};
		return { events, load };
	}

	it('uses the settings the page holds without reading them again', async () => {
		const { events, load } = recorder({ held: { mode: 'dub' } });
		await loadDiscoveryRows(load);
		expect(events.filter((e) => e.startsWith('ask'))).toEqual([
			'ask row 0 in dub',
			'ask row 1 in dub'
		]);
		expect(events).not.toContain('read settings');
	});

	it('reads and keeps the settings before asking for any row when the page has none', async () => {
		const { events, load } = recorder();
		await loadDiscoveryRows(load);
		expect(events.slice(0, 2)).toEqual(['read settings', 'keep dub']);
		expect(events).toContain('ask row 0 in dub');
		expect(events).toContain('ask row 1 in dub');
	});

	it('falls back to the default mode, keeping nothing, when the settings cannot be read', async () => {
		const { events, load } = recorder({
			readConfig: () => Promise.reject(new Error('offline'))
		});
		await loadDiscoveryRows(load);
		expect(events.some((e) => e.startsWith('keep'))).toBe(false);
		expect(events).toContain('ask row 0 in sub');
	});

	it('hands each row to its own slot, a failure as a failed row, and resolves once all settle', async () => {
		const rows: [number, RowLoad<string>][] = [];
		let answer: (items: string[]) => void = () => {};
		const held = new Promise<string[]>((resolve) => {
			answer = resolve;
		});
		let done = false;
		const loading = loadDiscoveryRows<string, Config>({
			config: () => null,
			readConfig: () => Promise.reject(new Error('offline')),
			keepConfig: () => {},
			pickMode,
			rows: [() => Promise.reject(new Error('503')), () => held],
			onRow: (i, row) => rows.push([i, row])
		}).then(() => (done = true));

		for (let k = 0; k < 10; k++) await Promise.resolve();
		expect(done).toBe(false);
		expect(rows).toEqual([[0, FAILED]]);

		answer(['r1']);
		await loading;
		expect(done).toBe(true);
		expect(new Map(rows)).toEqual(
			new Map<number, RowLoad<string>>([
				[0, FAILED],
				[1, ready('r1')]
			])
		);
	});

	it('settles every row exactly once into its own slot, whatever the outcomes', async () => {
		await fc.assert(
			fc.asyncProperty(
				fc.array(fc.option(fc.array(fc.string({ maxLength: 3 }), { maxLength: 3 })), {
					maxLength: 5
				}),
				fc.option(fc.constantFrom<'sub' | 'dub'>('sub', 'dub')),
				async (answers, heldMode) => {
					const seen: [number, RowLoad<string>][] = [];
					const asked: string[] = [];
					await loadDiscoveryRows<string, Config>({
						config: () => (heldMode === null ? null : { mode: heldMode }),
						readConfig: () => Promise.reject(new Error('offline')),
						keepConfig: () => {},
						pickMode,
						rows: answers.map((a) => (mode: 'sub' | 'dub') => {
							asked.push(mode);
							return a === null ? Promise.reject(new Error('x')) : Promise.resolve(a);
						}),
						onRow: (i, row) => seen.push([i, row])
					});
					expect(seen.map(([i]) => i).sort((x, y) => x - y)).toEqual(answers.map((_, i) => i));
					for (const [i, row] of seen) {
						const a = answers[i];
						expect(row).toEqual(a === null ? FAILED : ready(...a));
					}
					expect(new Set(asked).size).toBeLessThanOrEqual(1);
					if (asked.length > 0) expect(asked[0]).toBe(heldMode ?? 'sub');
				}
			)
		);
	});
});
