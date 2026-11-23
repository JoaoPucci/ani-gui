import { describe, expect, it, vi } from 'vitest';
import { followLink } from './link-reread';
import { latestRead } from './latest-read';
import type { WatchOrderAnswer } from './loader';

const settle = () => new Promise((r) => setTimeout(r, 0));
type Answer = WatchOrderAnswer<string>;

function harness(fresh: Answer | null) {
	const shown: (Answer | null)[] = [];
	const reads = latestRead<Answer | null>((d) => shown.push(d));
	const read = vi.fn(async () => fresh);
	return { shown, reads, read };
}

describe('followLink', () => {
	it('reads the page again, keeping what is shown meanwhile', async () => {
		const { shown, reads, read } = harness({ view: 'after the link' });
		followLink(reads, { view: 'before' }, read);
		expect(shown).toEqual([]);
		await settle();
		expect(read).toHaveBeenCalledOnce();
		expect(shown.at(-1)).toEqual({ view: 'after the link' });
	});

	it('keeps what is shown when the read again fails', async () => {
		for (const fresh of [{ view: null }, null]) {
			const { shown, reads, read } = harness(fresh);
			followLink(reads, { view: 'before' }, read);
			await settle();
			expect(shown).toEqual([{ view: 'before' }]);
		}
	});

	it('reads the show afresh when nothing is shown yet', async () => {
		const { shown, reads, read } = harness({ view: 'first' });
		followLink(reads, null, read);
		expect(shown).toEqual([null]);
		await settle();
		expect(read).toHaveBeenCalledOnce();
		expect(shown.at(-1)).toEqual({ view: 'first' });
	});

	it('leaves a page whose franchise could not be read as it is', async () => {
		const { shown, reads, read } = harness({ view: 'never' });
		followLink(reads, { view: null }, read);
		await settle();
		expect(read).not.toHaveBeenCalled();
		expect(shown).toEqual([]);
	});
});
