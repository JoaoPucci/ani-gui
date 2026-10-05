import { describe, expect, it } from 'vitest';
import { createRowTrust } from './row-trust';
import type { HistoryEntry, KitsuAnimeRef } from '$lib/api';

const row = (id: string): HistoryEntry => ({ id, ep_no: '1', title: 'Show' });
const ref = (id: string) => ({ id }) as unknown as KitsuAnimeRef;

describe('which Continue rows are matched by a guess', () => {
	it('notes a guessed match per row, and not a trusted one', async () => {
		const verdicts: Record<string, boolean> = { a: false, b: true };
		const trust = createRowTrust(async (e) => ({ match: ref('1'), trusted: verdicts[e.id] }));

		expect((await trust.resolveMatch(row('a')))?.id).toBe('1');
		await trust.resolveMatch(row('b'));

		// Both rows reach entry 1; the verdict belongs to the row.
		expect(trust.isGuess('a')).toBe(true);
		expect(trust.isGuess('b')).toBe(false);
	});

	it('takes a row re-resolved by a trusted match off the guesses', async () => {
		let trusted = false;
		const trust = createRowTrust(async () => ({ match: ref('1'), trusted }));
		await trust.resolveMatch(row('a'));
		trusted = true;
		await trust.resolveMatch(row('a'));

		expect(trust.isGuess('a')).toBe(false);
	});

	it('answers no guess for a row that resolved to no show', async () => {
		const trust = createRowTrust(async () => ({ match: null, trusted: false }));
		expect(await trust.resolveMatch(row('a'))).toBeNull();
		expect(trust.isGuess('a')).toBe(false);
	});
});
